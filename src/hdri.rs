// 3D Paintcraft - HDRI Environment Lighting & Image-Based Shading (IBL)
// Pure safe Rust for Linux creative studio workflows

use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdriPreset {
    StudioNeutral,       // Soft dual studio softboxes, neutral gray studio
    TomokoStudio,        // Clean photo studio with overhead diffused lighting
    CourtyardSunny,      // Warm direct outdoor sunlight and rich blue sky
    SunsetDesert,        // Golden hour warm low sun with purple-indigo twilight sky
    DarkIndustrial,      // Factory workshop with overhead fluorescent strip and steel bounce
    CyberpunkAlley,      // High-contrast dual neon magenta / cyan rim lights
    Custom(std::path::PathBuf),
}

impl HdriPreset {
    pub fn name(&self) -> String {
        match self {
            Self::StudioNeutral => "Studio Neutral (Softboxes)".to_string(),
            Self::TomokoStudio => "Tomoko Photo Studio".to_string(),
            Self::CourtyardSunny => "Courtyard Sunny (Outdoor)".to_string(),
            Self::SunsetDesert => "Sunset Desert (Golden Hour)".to_string(),
            Self::DarkIndustrial => "Dark Industrial Workshop".to_string(),
            Self::CyberpunkAlley => "Cyberpunk Alley (Neon)".to_string(),
            Self::Custom(path) => {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Custom HDRI");
                let clean = stem.replace('_', " ").replace('-', " ");
                let mut words: Vec<String> = Vec::new();
                for word in clean.split_whitespace() {
                    let mut c = word.chars();
                    if let Some(f) = c.next() {
                        words.push(f.to_uppercase().collect::<String>() + c.as_str());
                    }
                }
                if words.is_empty() {
                    clean
                } else {
                    words.join(" ")
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn all() -> &'static [HdriPreset] {
        &[
            HdriPreset::StudioNeutral,
            HdriPreset::TomokoStudio,
            HdriPreset::CourtyardSunny,
            HdriPreset::SunsetDesert,
            HdriPreset::DarkIndustrial,
            HdriPreset::CyberpunkAlley,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HdriEnvironment {
    pub preset: HdriPreset,
    pub rotation: f32, // Azimuth angle in degrees (0.0 to 360.0)
    pub exposure: f32, // Light intensity multiplier (0.1 to 3.0, default 1.0)
    #[serde(skip)]
    pub custom_hdr_pixels: Vec<Vec3>,
    pub custom_hdr_width: u32,
    pub custom_hdr_height: u32,
    pub custom_hdr_path: Option<std::path::PathBuf>,
}

impl Default for HdriEnvironment {
    fn default() -> Self {
        Self {
            preset: HdriPreset::StudioNeutral,
            rotation: 45.0,
            exposure: 1.0,
            custom_hdr_pixels: Vec::new(),
            custom_hdr_width: 0,
            custom_hdr_height: 0,
            custom_hdr_path: None,
        }
    }
}

impl HdriEnvironment {
    pub fn new(preset: HdriPreset) -> Self {
        Self {
            preset,
            rotation: 45.0,
            exposure: 1.0,
            custom_hdr_pixels: Vec::new(),
            custom_hdr_width: 0,
            custom_hdr_height: 0,
            custom_hdr_path: None,
        }
    }

    /// Load custom panoramic .hdr image from local filesystem
    pub fn load_custom_hdr(&mut self, path: &std::path::Path) -> Result<(), String> {
        use image::ImageDecoder;
        let file = std::fs::File::open(path).map_err(|e| format!("Failed to open HDR file: {}", e))?;
        let reader = std::io::BufReader::new(file);
        let decoder = image::codecs::hdr::HdrDecoder::new(reader).map_err(|e| format!("Invalid HDR format: {}", e))?;
        let (w, h) = decoder.dimensions();
        let total_bytes = (w as usize) * (h as usize) * 12;
        let mut byte_buf = vec![0u8; total_bytes];
        decoder.read_image(&mut byte_buf).map_err(|e| format!("Failed to decode HDR data: {}", e))?;

        self.custom_hdr_pixels = byte_buf.chunks_exact(12).map(|chunk| {
            let r = f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let g = f32::from_ne_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            let b = f32::from_ne_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
            Vec3::new(r, g, b)
        }).collect();
        self.custom_hdr_width = w;
        self.custom_hdr_height = h;
        self.custom_hdr_path = Some(path.to_path_buf());
        self.preset = HdriPreset::Custom(path.to_path_buf());
        Ok(())
    }

    /// Load panoramic environment image from file (.hdr, .png, .jpg, .exr)
    pub fn load_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if ext == "hdr" {
            self.load_custom_hdr(path)
        } else {
            let img = image::open(path).map_err(|e| format!("Failed to open image file: {}", e))?;
            let rgb = img.into_rgb32f();
            let (w, h) = rgb.dimensions();
            self.custom_hdr_pixels = rgb.pixels().map(|p| Vec3::new(p[0], p[1], p[2])).collect();
            self.custom_hdr_width = w;
            self.custom_hdr_height = h;
            self.custom_hdr_path = Some(path.to_path_buf());
            self.preset = HdriPreset::Custom(path.to_path_buf());
            Ok(())
        }
    }

    /// Sample equirectangular custom panoramic environment texture
    pub fn sample_equirectangular(&self, dir: Vec3) -> Option<Vec3> {
        if self.custom_hdr_pixels.is_empty() || self.custom_hdr_width == 0 || self.custom_hdr_height == 0 {
            return None;
        }
        let r_dir = self.rotate_dir(dir.normalize_or_zero());
        let u = (r_dir.z.atan2(r_dir.x) / (2.0 * std::f32::consts::PI) + 0.5).fract();
        let v = (0.5 - r_dir.y.clamp(-1.0, 1.0).asin() / std::f32::consts::PI).clamp(0.0, 1.0);
        let px = (u * (self.custom_hdr_width as f32 - 1.0)).clamp(0.0, self.custom_hdr_width as f32 - 1.0) as usize;
        let py = (v * (self.custom_hdr_height as f32 - 1.0)).clamp(0.0, self.custom_hdr_height as f32 - 1.0) as usize;
        let idx = py * self.custom_hdr_width as usize + px;
        self.custom_hdr_pixels.get(idx).copied()
    }

    /// Rotate world direction vector by the HDRI azimuth angle around Y axis
    #[inline]
    fn rotate_dir(&self, dir: Vec3) -> Vec3 {
        let rad = -self.rotation.to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();
        Vec3::new(
            dir.x * cos_a - dir.z * sin_a,
            dir.y,
            dir.x * sin_a + dir.z * cos_a,
        )
    }

    /// Primary key light direction derived from active HDRI rotation
    pub fn key_light_direction(&self) -> Vec3 {
        let local_sun = match &self.preset {
            HdriPreset::StudioNeutral => Vec3::new(0.6, 0.7, 0.4).normalize(),
            HdriPreset::TomokoStudio => Vec3::new(0.2, 0.9, 0.3).normalize(),
            HdriPreset::CourtyardSunny => Vec3::new(0.5, 0.8, 0.3).normalize(),
            HdriPreset::SunsetDesert => Vec3::new(0.8, 0.3, 0.5).normalize(),
            HdriPreset::DarkIndustrial => Vec3::new(0.3, 0.8, -0.5).normalize(),
            HdriPreset::CyberpunkAlley => Vec3::new(-0.7, 0.4, 0.5).normalize(),
            HdriPreset::Custom(_) => Vec3::new(0.5, 0.8, 0.3).normalize(),
        };

        // Rotate local sun by HDRI rotation
        let rad = self.rotation.to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();
        Vec3::new(
            local_sun.x * cos_a - local_sun.z * sin_a,
            local_sun.y,
            local_sun.x * sin_a + local_sun.z * cos_a,
        ).normalize()
    }

    /// Key light color and intensity
    pub fn key_light_color(&self) -> Vec3 {
        let col = match &self.preset {
            HdriPreset::StudioNeutral => Vec3::new(1.10, 1.08, 1.05) * 1.2,
            HdriPreset::TomokoStudio => Vec3::new(1.15, 1.15, 1.18) * 1.1,
            HdriPreset::CourtyardSunny => Vec3::new(1.30, 1.20, 1.00) * 1.4,
            HdriPreset::SunsetDesert => Vec3::new(1.45, 0.85, 0.45) * 1.5,
            HdriPreset::DarkIndustrial => Vec3::new(0.95, 1.05, 1.10) * 0.9,
            HdriPreset::CyberpunkAlley => Vec3::new(1.30, 0.20, 0.85) * 1.3,
            HdriPreset::Custom(_) => Vec3::new(1.0, 1.0, 1.0),
        };
        col * self.exposure
    }

    /// Evaluate diffuse irradiance for a surface normal
    pub fn evaluate_diffuse(&self, normal: Vec3) -> Vec3 {
        if let Some(col) = self.sample_equirectangular(normal) {
            return col * self.exposure;
        }

        let r_normal = self.rotate_dir(normal);
        let up_factor = (r_normal.y * 0.5 + 0.5).clamp(0.0, 1.0);

        let (sky, horizon, ground) = match &self.preset {
            HdriPreset::StudioNeutral => (
                Vec3::new(0.36, 0.38, 0.42),
                Vec3::new(0.24, 0.25, 0.28),
                Vec3::new(0.12, 0.12, 0.14),
            ),
            HdriPreset::TomokoStudio => (
                Vec3::new(0.42, 0.44, 0.48),
                Vec3::new(0.30, 0.31, 0.33),
                Vec3::new(0.16, 0.16, 0.18),
            ),
            HdriPreset::CourtyardSunny => (
                Vec3::new(0.25, 0.42, 0.70),
                Vec3::new(0.48, 0.52, 0.58),
                Vec3::new(0.20, 0.18, 0.14),
            ),
            HdriPreset::SunsetDesert => (
                Vec3::new(0.35, 0.18, 0.40),
                Vec3::new(0.65, 0.35, 0.18),
                Vec3::new(0.18, 0.10, 0.08),
            ),
            HdriPreset::DarkIndustrial => (
                Vec3::new(0.20, 0.24, 0.28),
                Vec3::new(0.15, 0.17, 0.20),
                Vec3::new(0.08, 0.08, 0.09),
            ),
            HdriPreset::CyberpunkAlley => (
                Vec3::new(0.10, 0.35, 0.55),
                Vec3::new(0.40, 0.10, 0.35),
                Vec3::new(0.06, 0.06, 0.08),
            ),
            HdriPreset::Custom(_) => (
                Vec3::new(0.5, 0.5, 0.5),
                Vec3::new(0.5, 0.5, 0.5),
                Vec3::new(0.5, 0.5, 0.5),
            ),
        };

        let hemi = if r_normal.y >= 0.0 {
            ground.lerp(sky, up_factor)
        } else {
            ground.lerp(horizon, up_factor)
        };

        hemi * self.exposure
    }

    pub fn evaluate_specular(&self, reflection_dir: Vec3, roughness: f32) -> Vec3 {
        let gloss = (1.0 - roughness).clamp(0.0, 1.0);
        if gloss <= 0.005 {
            return self.evaluate_diffuse(reflection_dir);
        }
        if let Some(col) = self.sample_equirectangular(reflection_dir) {
            let diff = self.evaluate_diffuse(reflection_dir);
            let spec = col * self.exposure;
            return diff.lerp(spec, gloss * gloss);
        }

        let r_dir = self.rotate_dir(reflection_dir);
        let r_clamped = roughness.clamp(0.015, 1.0);
        let gloss = 1.0 - r_clamped;

        // Specular power dynamically scales from high-power mirror pinpoints (3500) to soft diffuse (2)
        let spec_power = (2.0 / (r_clamped * r_clamped).max(0.0005) - 1.0).clamp(2.0, 3500.0);

        let (sky_color, horizon_color, ground_color) = match &self.preset {
            HdriPreset::StudioNeutral => (
                Vec3::new(0.55, 0.58, 0.65),
                Vec3::new(0.32, 0.34, 0.38),
                Vec3::new(0.08, 0.08, 0.10),
            ),
            HdriPreset::TomokoStudio => (
                Vec3::new(0.70, 0.72, 0.80),
                Vec3::new(0.42, 0.44, 0.48),
                Vec3::new(0.12, 0.12, 0.14),
            ),
            HdriPreset::CourtyardSunny => (
                Vec3::new(0.40, 0.65, 1.00),
                Vec3::new(0.65, 0.70, 0.75),
                Vec3::new(0.15, 0.13, 0.10),
            ),
            HdriPreset::SunsetDesert => (
                Vec3::new(0.60, 0.28, 0.65),
                Vec3::new(1.00, 0.58, 0.28),
                Vec3::new(0.12, 0.07, 0.05),
            ),
            HdriPreset::DarkIndustrial => (
                Vec3::new(0.32, 0.38, 0.45),
                Vec3::new(0.20, 0.24, 0.28),
                Vec3::new(0.06, 0.06, 0.07),
            ),
            HdriPreset::CyberpunkAlley => (
                Vec3::new(0.18, 0.65, 0.90),
                Vec3::new(0.90, 0.18, 0.70),
                Vec3::new(0.04, 0.04, 0.06),
            ),
            HdriPreset::Custom(_) => (
                Vec3::new(0.5, 0.5, 0.5),
                Vec3::new(0.5, 0.5, 0.5),
                Vec3::new(0.5, 0.5, 0.5),
            ),
        };

        // Smooth environment gradient (crisp horizon when smooth, blurred when rough)
        let up_factor = (r_dir.y * 0.5 + 0.5).clamp(0.0, 1.0);
        let curve_exp = 1.0 + gloss * 4.0;
        let env_factor = if r_dir.y >= 0.0 {
            up_factor.powf(1.0 / curve_exp)
        } else {
            up_factor.powf(curve_exp)
        };
        let base_env = if r_dir.y >= 0.0 {
            ground_color.lerp(sky_color, env_factor)
        } else {
            ground_color.lerp(horizon_color, env_factor)
        };

        // Add specular studio highlight hotspots
        let highlight = match &self.preset {
            HdriPreset::StudioNeutral => {
                let box1 = (r_dir.dot(Vec3::new(0.7, 0.6, 0.4).normalize())).max(0.0).powf(spec_power) * Vec3::new(3.5, 3.4, 3.2);
                let box2 = (r_dir.dot(Vec3::new(-0.6, 0.5, 0.6).normalize())).max(0.0).powf(spec_power * 0.75) * Vec3::new(2.5, 2.6, 2.8);
                box1 + box2
            }
            HdriPreset::TomokoStudio => {
                let top_panel = (r_dir.dot(Vec3::new(0.1, 0.95, 0.2).normalize())).max(0.0).powf(spec_power) * Vec3::new(4.2, 4.2, 4.4);
                top_panel
            }
            HdriPreset::CourtyardSunny => {
                let sun_disc = (r_dir.dot(Vec3::new(0.5, 0.8, 0.3).normalize())).max(0.0).powf(spec_power * 1.5) * Vec3::new(5.5, 5.0, 3.8);
                sun_disc
            }
            HdriPreset::SunsetDesert => {
                let sun_disc = (r_dir.dot(Vec3::new(0.8, 0.3, 0.5).normalize())).max(0.0).powf(spec_power * 1.3) * Vec3::new(6.0, 3.2, 1.6);
                sun_disc
            }
            HdriPreset::DarkIndustrial => {
                let strip = (r_dir.dot(Vec3::new(0.2, 0.85, -0.4).normalize())).max(0.0).powf(spec_power) * Vec3::new(3.2, 3.5, 3.8);
                strip
            }
            HdriPreset::CyberpunkAlley => {
                let neon_m = (r_dir.dot(Vec3::new(-0.7, 0.4, 0.5).normalize())).max(0.0).powf(spec_power) * Vec3::new(4.8, 0.5, 3.2);
                let neon_c = (r_dir.dot(Vec3::new(0.7, 0.3, -0.6).normalize())).max(0.0).powf(spec_power) * Vec3::new(0.4, 3.2, 4.8);
                neon_m + neon_c
            }
            HdriPreset::Custom(_) => Vec3::ZERO,
        };

        // When roughness approaches 0 (gloss -> 1), mirror reflection is intense and pin-sharp
        // When roughness is 1.0 (gloss -> 0), highlight is 0 and base_env smoothly equals evaluate_diffuse
        let diff_rad = self.evaluate_diffuse(reflection_dir);
        let spec_radiance = base_env.lerp(diff_rad, 1.0 - gloss) + highlight * (gloss * gloss);
        spec_radiance * self.exposure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hdr_encoder_available() {
        use image::ImageDecoder;
        let mut buf = Vec::new();
        let enc = image::codecs::hdr::HdrEncoder::new(&mut buf);
        let pixel = image::Rgb([1.5f32, 0.75f32, 0.25f32]);
        enc.encode(&[pixel], 1, 1).expect("Failed to encode HDR");
        assert!(!buf.is_empty());

        let reader = std::io::Cursor::new(buf);
        let decoder = image::codecs::hdr::HdrDecoder::new(reader).expect("Failed to read HDR");
        let (w, h) = decoder.dimensions();
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        let mut byte_buf = vec![0u8; 12];
        decoder.read_image(&mut byte_buf).expect("Failed to decode HDR");
        let r = f32::from_ne_bytes([byte_buf[0], byte_buf[1], byte_buf[2], byte_buf[3]]);
        let g = f32::from_ne_bytes([byte_buf[4], byte_buf[5], byte_buf[6], byte_buf[7]]);
        let b = f32::from_ne_bytes([byte_buf[8], byte_buf[9], byte_buf[10], byte_buf[11]]);
        assert!((r - 1.5).abs() < 0.05);
        assert!((g - 0.75).abs() < 0.05);
        assert!((b - 0.25).abs() < 0.05);

        let env = HdriEnvironment::new(HdriPreset::StudioNeutral);
        assert_eq!(env.preset.name(), "Studio Neutral (Softboxes)");
    }
}

