use crate::hdri::HdriEnvironment;
use rayon::prelude::*;
use std::f32::consts::PI;

pub fn generate_hdri_texture(hdri: &HdriEnvironment, width: u32, height: u32) -> (u32, u32, Vec<f32>) {
    // 1. If HDRI already has loaded pixels in memory (.hdr, .png, etc.)
    if !hdri.custom_hdr_pixels.is_empty() && hdri.custom_hdr_width > 0 && hdri.custom_hdr_height > 0 {
        let w = hdri.custom_hdr_width;
        let h = hdri.custom_hdr_height;
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        let exp = hdri.exposure;
        for p in &hdri.custom_hdr_pixels {
            data.push(p.x * exp);
            data.push(p.y * exp);
            data.push(p.z * exp);
            data.push(1.0);
        }
        return (w, h, data);
    }

    // 2. If it's a Custom file path that needs loading
    if let crate::hdri::HdriPreset::Custom(path) = &hdri.preset {
        let mut temp_env = hdri.clone();
        if temp_env.load_file(path).is_ok() && !temp_env.custom_hdr_pixels.is_empty() {
            let w = temp_env.custom_hdr_width;
            let h = temp_env.custom_hdr_height;
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            let exp = temp_env.exposure;
            for p in &temp_env.custom_hdr_pixels {
                data.push(p.x * exp);
                data.push(p.y * exp);
                data.push(p.z * exp);
                data.push(1.0);
            }
            return (w, h, data);
        }
    }

    let mut data = vec![0.0f32; (width * height * 4) as usize];
    
    data.par_chunks_exact_mut(4).enumerate().for_each(|(idx, pixel)| {
        let x = (idx as u32) % width;
        let y = (idx as u32) / width;
        
        let u = (x as f32 + 0.5) / width as f32;
        let v = (y as f32 + 0.5) / height as f32;
        
        let theta = u * 2.0 * PI - PI;
        let phi = PI * 0.5 - v * PI;
        
        let dir = glam::Vec3::new(
            phi.cos() * theta.cos(),
            phi.sin(),
            phi.cos() * theta.sin(),
        );
        
        // Evaluate at roughness 0.08 for the base HDRI to provide physical softbox extent across mip levels
        let color = hdri.evaluate_specular(dir, 0.08);
        
        pixel[0] = color.x;
        pixel[1] = color.y;
        pixel[2] = color.z;
        pixel[3] = 1.0;
    });
    
    (width, height, data)
}
