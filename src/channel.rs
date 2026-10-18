// 3D Paintcraft - PBR Texture Channels and Buffers
// Pure safe Rust

use glam::{Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelType {
    BaseColor,
    Roughness,
    Metallic,
    Normal,
    Height,
    AmbientOcclusion,
    Emission,
    Opacity,
    Clearcoat,
    Anisotropy,
}

#[allow(dead_code)]
impl ChannelType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::BaseColor => "Base Color",
            Self::Roughness => "Roughness",
            Self::Metallic => "Metallic",
            Self::Normal => "Normal",
            Self::Height => "Height",
            Self::AmbientOcclusion => "Ambient Occlusion",
            Self::Emission => "Emission",
            Self::Opacity => "Opacity",
            Self::Clearcoat => "Clearcoat",
            Self::Anisotropy => "Anisotropy",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::BaseColor => "Color",
            Self::Roughness => "Rough",
            Self::Metallic => "Metal",
            Self::Normal => "Norm",
            Self::Height => "Height",
            Self::AmbientOcclusion => "AO",
            Self::Emission => "Emis",
            Self::Opacity => "Opac",
            Self::Clearcoat => "Coat",
            Self::Anisotropy => "Aniso",
        }
    }
}

/// Channel enable bitmask / flags
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelFlags {
    pub base_color: bool,
    pub roughness: bool,
    pub metallic: bool,
    pub normal: bool,
    pub height: bool,
    pub ao: bool,
    pub emission: bool,
    pub opacity: bool,
    pub clearcoat: bool,
    pub anisotropy: bool,
}

impl Default for ChannelFlags {
    fn default() -> Self {
        Self {
            base_color: true,
            roughness: true,
            metallic: true,
            normal: false,
            height: false,
            ao: false,
            emission: false,
            opacity: false,
            clearcoat: false,
            anisotropy: false,
        }
    }
}

impl ChannelFlags {
    #[allow(dead_code)]
    pub fn all() -> Self {
        Self {
            base_color: true,
            roughness: true,
            metallic: true,
            normal: true,
            height: true,
            ao: true,
            emission: true,
            opacity: true,
            clearcoat: true,
            anisotropy: true,
        }
    }
}

/// 2D RGBA 32-bit floating point image buffer
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RgbaBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<Vec4>,
}

impl RgbaBuffer {
    pub fn new(width: u32, height: u32, default_color: Vec4) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            data: vec![default_color; size],
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> Vec4 {
        if x < self.width && y < self.height {
            self.data[(y * self.width + x) as usize]
        } else {
            Vec4::ZERO
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Vec4) {
        if x < self.width && y < self.height {
            self.data[(y * self.width + x) as usize] = color;
        }
    }

    /// Sample with bilinear interpolation in UV coordinates [0.0, 1.0]
    pub fn sample_bilinear(&self, uv: Vec2) -> Vec4 {
        let u = (uv.x.rem_euclid(1.0)) * (self.width as f32);
        let v = (uv.y.rem_euclid(1.0)) * (self.height as f32);

        let x0 = (u.floor() as u32).min(self.width - 1);
        let y0 = (v.floor() as u32).min(self.height - 1);
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let fx = u - u.floor();
        let fy = v - v.floor();

        let c00 = self.get_pixel(x0, y0);
        let c10 = self.get_pixel(x1, y0);
        let c01 = self.get_pixel(x0, y1);
        let c11 = self.get_pixel(x1, y1);

        let c0 = c00 * (1.0 - fx) + c10 * fx;
        let c1 = c01 * (1.0 - fx) + c11 * fx;

        c0 * (1.0 - fy) + c1 * fy
    }

    /// Resize buffer with bilinear filtering
    pub fn resize_bilinear(&mut self, new_width: u32, new_height: u32) {
        if self.width == new_width && self.height == new_height {
            return;
        }
        let mut new_data = Vec::with_capacity((new_width * new_height) as usize);
        for y in 0..new_height {
            for x in 0..new_width {
                let uv = Vec2::new(
                    (x as f32 + 0.5) / new_width as f32,
                    (y as f32 + 0.5) / new_height as f32,
                );
                new_data.push(self.sample_bilinear(uv));
            }
        }
        self.width = new_width;
        self.height = new_height;
        self.data = new_data;
    }

    /// Load an RGBA buffer from an image file (png, jpg, etc.)
    pub fn from_image_file(path: &std::path::Path) -> Result<Self, String> {
        let img = image::open(path).map_err(|e| format!("Failed to load image: {}", e))?;
        let rgba = img.to_rgba8();
        let width = rgba.width();
        let height = rgba.height();
        let data: Vec<Vec4> = rgba.pixels().map(|p| {
            Vec4::new(
                p[0] as f32 / 255.0,
                p[1] as f32 / 255.0,
                p[2] as f32 / 255.0,
                p[3] as f32 / 255.0,
            )
        }).collect();
        Ok(Self { width, height, data })
    }

    /// Separable 1D Gaussian blur
    pub fn filter_gaussian_blur(&mut self, radius: f32) {
        if radius < 0.5 { return; }
        let w = self.width as usize;
        let h = self.height as usize;
        let r = radius.ceil() as i32;
        let sigma = (radius * 0.5).max(0.5);
        let mut kernel = Vec::with_capacity((r * 2 + 1) as usize);
        let mut sum = 0.0f32;
        for i in -r..=r {
            let wt = (-((i * i) as f32) / (2.0 * sigma * sigma)).exp();
            kernel.push(wt);
            sum += wt;
        }
        for k in &mut kernel { *k /= sum; }

        let mut temp = self.data.clone();
        for y in 0..h {
            for x in 0..w {
                let mut acc = Vec4::ZERO;
                for (k_idx, &wt) in kernel.iter().enumerate() {
                    let offset = k_idx as i32 - r;
                    let nx = (x as i32 + offset).clamp(0, w as i32 - 1) as usize;
                    acc += self.data[y * w + nx] * wt;
                }
                temp[y * w + x] = acc;
            }
        }
        for y in 0..h {
            for x in 0..w {
                let mut acc = Vec4::ZERO;
                for (k_idx, &wt) in kernel.iter().enumerate() {
                    let offset = k_idx as i32 - r;
                    let ny = (y as i32 + offset).clamp(0, h as i32 - 1) as usize;
                    acc += temp[ny * w + x] * wt;
                }
                self.data[y * w + x] = acc;
            }
        }
    }

    /// Levels adjustment on RGB channels
    pub fn filter_levels(&mut self, in_black: f32, in_white: f32, gamma: f32, out_black: f32, out_white: f32) {
        let inv_range = 1.0 / (in_white - in_black).max(1e-4);
        let inv_gamma = 1.0 / gamma.max(0.01);
        let out_range = out_white - out_black;
        for c in &mut self.data {
            let mut rgb = [c.x, c.y, c.z];
            for v in &mut rgb {
                let norm = ((*v - in_black) * inv_range).clamp(0.0, 1.0);
                let corrected = norm.powf(inv_gamma);
                *v = out_black + corrected * out_range;
            }
            *c = Vec4::new(rgb[0], rgb[1], rgb[2], c.w);
        }
    }

    /// Invert RGB channels
    pub fn filter_invert(&mut self) {
        for c in &mut self.data {
            *c = Vec4::new(1.0 - c.x, 1.0 - c.y, 1.0 - c.z, c.w);
        }
    }

    /// Sharpen via unsharp mask
    pub fn filter_sharpen(&mut self, strength: f32) {
        let mut blurred = self.clone();
        blurred.filter_gaussian_blur(1.5);
        for (orig, blur) in self.data.iter_mut().zip(blurred.data.iter()) {
            let diff = orig.truncate() - blur.truncate();
            let sharp = (orig.truncate() + diff * strength).clamp(Vec3::ZERO, Vec3::ONE);
            *orig = Vec4::new(sharp.x, sharp.y, sharp.z, orig.w);
        }
    }

    /// Contrast and brightness adjustment on RGB channels
    pub fn filter_contrast_brightness(&mut self, contrast: f32, brightness: f32) {
        for c in &mut self.data {
            let rgb = c.truncate();
            let with_b = (rgb + Vec3::splat(brightness)).clamp(Vec3::ZERO, Vec3::ONE);
            let contrasted = ((with_b - Vec3::splat(0.5)) * contrast + Vec3::splat(0.5)).clamp(Vec3::ZERO, Vec3::ONE);
            *c = Vec4::new(contrasted.x, contrasted.y, contrasted.z, c.w);
        }
    }

    /// High-pass filter isolating high-frequency detail
    pub fn filter_high_pass(&mut self, radius: f32) {
        let mut blurred = self.clone();
        blurred.filter_gaussian_blur(radius);
        for (orig, blur) in self.data.iter_mut().zip(blurred.data.iter()) {
            let diff = orig.truncate() - blur.truncate() + Vec3::splat(0.5);
            let clamped = diff.clamp(Vec3::ZERO, Vec3::ONE);
            *orig = Vec4::new(clamped.x, clamped.y, clamped.z, orig.w);
        }
    }
}

/// 2D Grayscale / Scalar 32-bit floating point image buffer
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScalarBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f32>,
}

impl ScalarBuffer {
    pub fn new(width: u32, height: u32, default_value: f32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            data: vec![default_value; size],
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> f32 {
        if x < self.width && y < self.height {
            self.data[(y * self.width + x) as usize]
        } else {
            0.0
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, value: f32) {
        if x < self.width && y < self.height {
            self.data[(y * self.width + x) as usize] = value;
        }
    }

    pub fn invert(&mut self) {
        for v in &mut self.data {
            *v = (1.0 - *v).clamp(0.0, 1.0);
        }
    }

    pub fn sample_bilinear(&self, uv: Vec2) -> f32 {
        let u = (uv.x.rem_euclid(1.0)) * (self.width as f32);
        let v = (uv.y.rem_euclid(1.0)) * (self.height as f32);

        let x0 = (u.floor() as u32).min(self.width - 1);
        let y0 = (v.floor() as u32).min(self.height - 1);
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let fx = u - u.floor();
        let fy = v - v.floor();

        let c00 = self.get_pixel(x0, y0);
        let c10 = self.get_pixel(x1, y0);
        let c01 = self.get_pixel(x0, y1);
        let c11 = self.get_pixel(x1, y1);

        let c0 = c00 * (1.0 - fx) + c10 * fx;
        let c1 = c01 * (1.0 - fx) + c11 * fx;

        c0 * (1.0 - fy) + c1 * fy
    }

    /// Resize buffer with bilinear filtering
    pub fn resize_bilinear(&mut self, new_width: u32, new_height: u32) {
        if self.width == new_width && self.height == new_height {
            return;
        }
        let mut new_data = Vec::with_capacity((new_width * new_height) as usize);
        for y in 0..new_height {
            for x in 0..new_width {
                let uv = Vec2::new(
                    (x as f32 + 0.5) / new_width as f32,
                    (y as f32 + 0.5) / new_height as f32,
                );
                new_data.push(self.sample_bilinear(uv));
            }
        }
        self.width = new_width;
        self.height = new_height;
        self.data = new_data;
    }

    /// Load a scalar buffer from an image file (uses luminance / grayscale)
    #[allow(dead_code)]
    pub fn from_image_file(path: &std::path::Path) -> Result<Self, String> {
        let img = image::open(path).map_err(|e| format!("Failed to load image: {}", e))?;
        let gray = img.to_luma8();
        let width = gray.width();
        let height = gray.height();
        let data: Vec<f32> = gray.pixels().map(|p| p[0] as f32 / 255.0).collect();
        Ok(Self { width, height, data })
    }

    /// Separable 1D Gaussian blur
    pub fn filter_gaussian_blur(&mut self, radius: f32) {
        if radius < 0.5 { return; }
        let w = self.width as usize;
        let h = self.height as usize;
        let r = radius.ceil() as i32;
        let sigma = (radius * 0.5).max(0.5);
        let mut kernel = Vec::with_capacity((r * 2 + 1) as usize);
        let mut sum = 0.0f32;
        for i in -r..=r {
            let wt = (-((i * i) as f32) / (2.0 * sigma * sigma)).exp();
            kernel.push(wt);
            sum += wt;
        }
        for k in &mut kernel { *k /= sum; }

        let mut temp = self.data.clone();
        for y in 0..h {
            for x in 0..w {
                let mut acc = 0.0f32;
                for (k_idx, &wt) in kernel.iter().enumerate() {
                    let offset = k_idx as i32 - r;
                    let nx = (x as i32 + offset).clamp(0, w as i32 - 1) as usize;
                    acc += self.data[y * w + nx] * wt;
                }
                temp[y * w + x] = acc;
            }
        }
        for y in 0..h {
            for x in 0..w {
                let mut acc = 0.0f32;
                for (k_idx, &wt) in kernel.iter().enumerate() {
                    let offset = k_idx as i32 - r;
                    let ny = (y as i32 + offset).clamp(0, h as i32 - 1) as usize;
                    acc += temp[ny * w + x] * wt;
                }
                self.data[y * w + x] = acc;
            }
        }
    }

    /// Levels adjustment
    pub fn filter_levels(&mut self, in_black: f32, in_white: f32, gamma: f32, out_black: f32, out_white: f32) {
        let inv_range = 1.0 / (in_white - in_black).max(1e-4);
        let inv_gamma = 1.0 / gamma.max(0.01);
        let out_range = out_white - out_black;
        for v in &mut self.data {
            let norm = ((*v - in_black) * inv_range).clamp(0.0, 1.0);
            let corrected = norm.powf(inv_gamma);
            *v = out_black + corrected * out_range;
        }
    }

    /// Invert values
    pub fn filter_invert(&mut self) {
        for v in &mut self.data {
            *v = 1.0 - *v;
        }
    }

    /// Sharpen via unsharp mask
    pub fn filter_sharpen(&mut self, strength: f32) {
        let mut blurred = self.clone();
        blurred.filter_gaussian_blur(1.5);
        for (orig, blur) in self.data.iter_mut().zip(blurred.data.iter()) {
            let diff = *orig - *blur;
            *orig = (*orig + diff * strength).clamp(0.0, 1.0);
        }
    }

    /// Contrast and brightness adjustment on scalar values
    pub fn filter_contrast_brightness(&mut self, contrast: f32, brightness: f32) {
        for v in &mut self.data {
            let with_b = (*v + brightness).clamp(0.0, 1.0);
            *v = ((with_b - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
        }
    }

    /// High-pass filter isolating high-frequency detail
    pub fn filter_high_pass(&mut self, radius: f32) {
        let mut blurred = self.clone();
        blurred.filter_gaussian_blur(radius);
        for (orig, blur) in self.data.iter_mut().zip(blurred.data.iter()) {
            let diff = (*orig - *blur) + 0.5;
            *orig = diff.clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_buffer_invert() {
        let mut buf = ScalarBuffer::new(2, 2, 0.25);
        buf.set_pixel(1, 0, 0.0);
        buf.set_pixel(0, 1, 1.0);
        buf.invert();

        assert!((buf.get_pixel(0, 0) - 0.75).abs() < 1e-5);
        assert!((buf.get_pixel(1, 0) - 1.0).abs() < 1e-5);
        assert!((buf.get_pixel(0, 1) - 0.0).abs() < 1e-5);
    }
}
