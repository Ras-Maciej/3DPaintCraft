// 3D Paintcraft - Interactive Progressive Monte Carlo Path Tracer (Iray Mode)
// Pure safe Rust for Linux creative studio workflows

use glam::Vec3;
use rayon::prelude::*;
use crate::hdri::HdriEnvironment;
use crate::layer::CompositeTextureSet;
use crate::mesh::{Mesh, Ray};
use crate::renderer::Camera;

/// Fast pseudo-random number generator for Monte Carlo sampling
#[derive(Clone, Copy, Debug)]
pub struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    #[inline]
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16777216.0
    }
}

/// Cosine-weighted hemisphere sampling for diffuse Lambertian indirect bounce
#[inline]
pub fn sample_cosine_hemisphere(normal: Vec3, rng: &mut XorShift64) -> Vec3 {
    let u1 = rng.next_f32();
    let u2 = rng.next_f32();
    let r = u1.sqrt();
    let theta = 2.0 * std::f32::consts::PI * u2;
    let x = r * theta.cos();
    let y = r * theta.sin();
    let z = (1.0 - u1).max(0.0).sqrt();

    let up = if normal.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
    let tangent = normal.cross(up).normalize_or_zero();
    let bitangent = normal.cross(tangent);

    (tangent * x + bitangent * y + normal * z).normalize_or_zero()
}

/// Importance sample GGX normal distribution for a given surface normal and roughness
#[inline]
pub fn sample_ggx(normal: Vec3, roughness: f32, rng: &mut XorShift64) -> Vec3 {
    let alpha = (roughness * roughness).clamp(0.001, 1.0);
    let u1 = rng.next_f32();
    let u2 = rng.next_f32();

    let phi = 2.0 * std::f32::consts::PI * u1;
    let cos_theta = ((1.0 - u2) / (1.0 + (alpha * alpha - 1.0) * u2).max(1e-6))
        .sqrt()
        .clamp(0.0, 1.0);
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();

    let h_local = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);

    let up = if normal.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
    let tangent = normal.cross(up).normalize_or_zero();
    let bitangent = normal.cross(tangent);

    (tangent * h_local.x + bitangent * h_local.y + normal * h_local.z).normalize_or_zero()
}

/// ACES Filmic tonemapping curve (Unreal Engine 5 / Substance 3D standard)
#[inline]
pub fn tonemap_aces(v: Vec3) -> Vec3 {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    ((v * (a * v + b)) / (v * (c * v + d) + e)).clamp(Vec3::ZERO, Vec3::ONE)
}

/// Progressive Monte Carlo Path Tracer
#[derive(Clone, Debug)]
pub struct PathTracer {
    pub width: u32,
    pub height: u32,
    pub accumulation_buffer: Vec<Vec3>,
    pub sample_count: u32,
    pub max_samples: u32,
    pub max_bounces: u32,
    pub color_buffer: Vec<u8>, // RGBA8 format
}

impl PathTracer {
    pub fn new(width: u32, height: u32) -> Self {
        let count = (width * height) as usize;
        Self {
            width,
            height,
            accumulation_buffer: vec![Vec3::ZERO; count],
            sample_count: 0,
            max_samples: 256,
            max_bounces: 2,
            color_buffer: vec![0u8; count * 4],
        }
    }

    /// Resize accumulation and color buffers if viewport dimensions changed
    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        if self.width != new_width || self.height != new_height {
            self.width = new_width;
            self.height = new_height;
            let count = (new_width * new_height) as usize;
            self.accumulation_buffer = vec![Vec3::ZERO; count];
            self.color_buffer = vec![0u8; count * 4];
            self.sample_count = 0;
        }
    }

    /// Reset accumulation when camera, scene, or materials change
    pub fn reset(&mut self) {
        self.accumulation_buffer.fill(Vec3::ZERO);
        self.sample_count = 0;
    }

    /// Check if target convergence has been reached
    pub fn is_converged(&self) -> bool {
        self.sample_count >= self.max_samples
    }

    /// Progress percentage towards max_samples
    pub fn progress_pct(&self) -> f32 {
        if self.max_samples == 0 {
            100.0
        } else {
            ((self.sample_count as f32 / self.max_samples as f32).min(1.0)) * 100.0
        }
    }

    /// Render progressive samples across all scanlines in parallel via Rayon
    pub fn render_samples(
        &mut self,
        camera: &Camera,
        mesh: &Mesh,
        textures: &CompositeTextureSet,
        hdri: &HdriEnvironment,
        samples_per_frame: u32,
    ) {
        if self.width == 0 || self.height == 0 || self.is_converged() {
            return;
        }

        let w = self.width;
        let h = self.height;
        let total_pixels = (w * h) as usize;
        if self.accumulation_buffer.len() != total_pixels {
            self.accumulation_buffer.resize(total_pixels, Vec3::ZERO);
            self.color_buffer.resize(total_pixels * 4, 0);
        }

        let current_sample = self.sample_count;
        let samples_to_run = samples_per_frame.min(self.max_samples.saturating_sub(current_sample));
        if samples_to_run == 0 {
            return;
        }

        let max_bounces = self.max_bounces;
        let w_f32 = w as f32;
        let h_f32 = h as f32;

        self.accumulation_buffer
            .par_chunks_mut(w as usize)
            .enumerate()
            .for_each(|(y, scanline)| {
                for (x, accum) in scanline.iter_mut().enumerate() {
                    let mut rng = XorShift64::new(
                        ((y as u64) * (w as u64) + (x as u64) + 1)
                            ^ (((current_sample + 1) as u64).wrapping_mul(0x9e3779b97f4a7c15)),
                    );

                    for _ in 0..samples_to_run {
                        let subpixel_x = x as f32 + rng.next_f32();
                        let subpixel_y = y as f32 + rng.next_f32();
                        let primary_ray = camera.screen_to_ray(subpixel_x, subpixel_y, w_f32, h_f32);

                        let sample_color = trace_path(
                            primary_ray,
                            mesh,
                            textures,
                            hdri,
                            max_bounces,
                            &mut rng,
                        );

                        *accum += sample_color;
                    }
                }
            });

        self.sample_count += samples_to_run;
        let s_count_f32 = self.sample_count.max(1) as f32;

        // ACES Filmic tonemapping and display sRGB conversion into color_buffer
        let accum_buf = &self.accumulation_buffer;
        self.color_buffer
            .par_chunks_exact_mut(4)
            .enumerate()
            .for_each(|(idx, rgba)| {
                let mean_rad = accum_buf[idx] / s_count_f32;

                // ACES Filmic tonemapping
                let mapped = tonemap_aces(mean_rad);

                // Convert linear photometric radiance to display sRGB
                let inv_gamma = 1.0 / 2.2;
                let r = (mapped.x.clamp(0.0, 1.0).powf(inv_gamma) * 255.0 + 0.5) as u8;
                let g = (mapped.y.clamp(0.0, 1.0).powf(inv_gamma) * 255.0 + 0.5) as u8;
                let b = (mapped.z.clamp(0.0, 1.0).powf(inv_gamma) * 255.0 + 0.5) as u8;

                rgba[0] = r;
                rgba[1] = g;
                rgba[2] = b;
                rgba[3] = 255;
            });
    }

    /// Save converged path-traced viewport image to disk as PNG
    pub fn save_to_png(&self, path: &std::path::Path) -> Result<(), String> {
        if self.width == 0 || self.height == 0 || self.color_buffer.is_empty() {
            return Err("Path tracer color buffer is empty".to_string());
        }
        image::save_buffer(
            path,
            &self.color_buffer,
            self.width,
            self.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to save image: {}", e))
    }
}

/// Trace a single light transport path with physical GGX microfacets, shadow rays, and environment IBL
#[inline]
fn trace_path(
    initial_ray: Ray,
    mesh: &Mesh,
    textures: &CompositeTextureSet,
    hdri: &HdriEnvironment,
    max_bounces: u32,
    rng: &mut XorShift64,
) -> Vec3 {
    let mut ray = initial_ray;
    let mut throughput = Vec3::ONE;
    let mut radiance = Vec3::ZERO;

    for bounce in 0..=max_bounces {
        let hit_opt = mesh.bvh.raycast(&ray, &mesh.vertices, &mesh.indices);

        let hit = match hit_opt {
            Some(h) => h,
            None => {
                // Ray missed into environment / HDRI sky
                let sky_radiance = if let Some(env_col) = hdri.sample_equirectangular(ray.direction) {
                    env_col * hdri.exposure
                } else {
                    hdri.evaluate_specular(ray.direction, 0.0)
                };
                radiance += throughput * sky_radiance;
                break;
            }
        };

        // Surface hit interpolation
        let w = hit.barycentric.x;
        let u = hit.barycentric.y;
        let v = hit.barycentric.z;

        let tri_base = hit.triangle_idx * 3;
        if tri_base + 2 >= mesh.indices.len() {
            break;
        }

        let i0 = mesh.indices[tri_base] as usize;
        let i1 = mesh.indices[tri_base + 1] as usize;
        let i2 = mesh.indices[tri_base + 2] as usize;

        if i0 >= mesh.vertices.len() || i1 >= mesh.vertices.len() || i2 >= mesh.vertices.len() {
            break;
        }

        let v0 = &mesh.vertices[i0];
        let v1 = &mesh.vertices[i1];
        let v2 = &mesh.vertices[i2];

        let geom_normal = (v0.normal * w + v1.normal * u + v2.normal * v).normalize_or_zero();
        let tangent = (v0.tangent.truncate() * w + v1.tangent.truncate() * u + v2.tangent.truncate() * v).normalize_or_zero();
        let bitangent = (geom_normal.cross(tangent) * v0.tangent.w).normalize_or_zero();

        // Sample PBR material maps at UV coordinate
        let uv = hit.uv;

        // Convert base color from stored sRGB to linear radiometric space for physical transport
        let base_color = textures.base_color.sample_bilinear(uv).truncate();
        let albedo = Vec3::new(
            base_color.x.powf(2.2),
            base_color.y.powf(2.2),
            base_color.z.powf(2.2),
        );

        let metallic = textures.metallic.sample_bilinear(uv).clamp(0.0, 1.0);
        let roughness = textures.roughness.sample_bilinear(uv).clamp(0.015, 1.0);
        let ao = textures.ao.sample_bilinear(uv).clamp(0.0, 1.0);

        let emissive = textures.emissive.sample_bilinear(uv).truncate();
        let emissive_lin = Vec3::new(
            emissive.x.powf(2.2),
            emissive.y.powf(2.2),
            emissive.z.powf(2.2),
        );

        // Tangent-space normal mapping
        let norm_sample = textures.normal.sample_bilinear(uv);
        let n_tan = Vec3::new(
            norm_sample.x * 2.0 - 1.0,
            norm_sample.y * 2.0 - 1.0,
            (norm_sample.z * 2.0 - 1.0).max(0.01),
        );

        let shaded_normal = if tangent.length_squared() > 0.01 && bitangent.length_squared() > 0.01 {
            (tangent * n_tan.x + bitangent * n_tan.y + geom_normal * n_tan.z).normalize_or_zero()
        } else {
            geom_normal
        };

        // Ensure normal faces incident ray
        let normal = if shaded_normal.dot(-ray.direction) < 0.0 {
            -shaded_normal
        } else {
            shaded_normal
        };

        let hit_pos = hit.point + normal * 0.0005;
        let v_dir = -ray.direction;
        let n_dot_v = normal.dot(v_dir).max(1e-4);

        // Dielectric F0 is 0.04, metal F0 is tinted by linear albedo
        let f0 = Vec3::splat(0.04).lerp(albedo, metallic);

        // Physical Matte Attenuation:
        // As roughness approaches 1.0 on dielectrics, microfacets completely scatter specular into diffuse
        let gloss = (1.0 - roughness).clamp(0.0, 1.0);
        let dielectric_falloff = gloss.powf(1.5);
        let matte_factor = dielectric_falloff * (1.0 - metallic) + metallic;

        // 1. Direct Emissive illumination
        radiance += throughput * emissive_lin;

        // 2. Direct Key / Sun Light with Shadow Ray (True Contact Shadows)
        let l_dir = hdri.key_light_direction();
        let n_dot_l = normal.dot(l_dir).max(0.0);

        if n_dot_l > 0.0 {
            let shadow_ray = Ray::new(hit_pos, l_dir);
            let in_shadow = mesh.bvh.raycast(&shadow_ray, &mesh.vertices, &mesh.indices).is_some();

            if !in_shadow {
                let h_sun = (v_dir + l_dir).normalize_or_zero();
                let n_dot_h = normal.dot(h_sun).max(0.0);
                let v_dot_h = v_dir.dot(h_sun).max(0.0);

                // Fresnel Schlick
                let f_sun = f0 + (Vec3::ONE - f0) * (1.0 - v_dot_h).powf(5.0);

                // GGX Normal Distribution
                let alpha = (roughness * roughness).max(0.001);
                let alpha2 = alpha * alpha;
                let denom_d = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
                let d = alpha2 / (std::f32::consts::PI * denom_d * denom_d).max(1e-8);

                // Smith Geometry Function
                let k = (roughness + 1.0).powi(2) / 8.0;
                let g1_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
                let g1_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
                let g = g1_v * g1_l;

                let spec = (d * g) / (4.0 * n_dot_v * n_dot_l).max(1e-4) * f_sun * matte_factor;
                let diff = (Vec3::ONE - f_sun) * (1.0 - metallic) * albedo / std::f32::consts::PI;

                let light_color = hdri.key_light_color();
                radiance += throughput * (diff + spec) * light_color * n_dot_l * ao;
            }
        }

        // 3. Environment Diffuse Irradiance (Ambient Light)
        let diffuse_irradiance = hdri.evaluate_diffuse(normal);
        let ibl_diffuse = (Vec3::ONE - f0) * (1.0 - metallic) * albedo * diffuse_irradiance * ao;
        radiance += throughput * ibl_diffuse;

        // 4. Environment Specular Reflection via GGX Microfacet Importance Sampling
        // Samples the HDRI along the physical microfacet distribution parameterized by roughness
        let h_microfacet = sample_ggx(normal, roughness, rng);
        let r_spec = (2.0 * v_dir.dot(h_microfacet) * h_microfacet - v_dir).normalize_or_zero();

        if r_spec.dot(normal) > 0.0 {
            let v_dot_hm = v_dir.dot(h_microfacet).max(0.0);
            let f_spec = f0 + (Vec3::ONE - f0) * (1.0 - v_dot_hm).powf(5.0);

            // Cast specular ray to test for self-reflection vs environment
            let spec_ray = Ray::new(hit_pos, r_spec);
            let spec_hit = mesh.bvh.raycast(&spec_ray, &mesh.vertices, &mesh.indices);

            let spec_incoming = match spec_hit {
                Some(sh) => {
                    // Reflected light from another surface on the mesh
                    let s_uv = sh.uv;
                    let s_base = textures.base_color.sample_bilinear(s_uv).truncate();
                    let s_albedo = Vec3::new(s_base.x.powf(2.2), s_base.y.powf(2.2), s_base.z.powf(2.2));
                    let s_diff = hdri.evaluate_diffuse(sh.normal);
                    s_albedo * s_diff * 0.5
                }
                None => {
                    // Reflected light from the HDRI environment
                    if let Some(c) = hdri.sample_equirectangular(r_spec) {
                        c * hdri.exposure
                    } else {
                        hdri.evaluate_specular(r_spec, roughness)
                    }
                }
            };

            radiance += throughput * spec_incoming * f_spec * matte_factor * ao;
        }

        // 5. Clearcoat specular reflection layer
        let clearcoat = textures.clearcoat.sample_bilinear(uv);
        if clearcoat > 0.01 {
            let cc_h = sample_ggx(normal, 0.08, rng);
            let cc_r = (2.0 * v_dir.dot(cc_h) * cc_h - v_dir).normalize_or_zero();
            if cc_r.dot(normal) > 0.0 {
                let cc_sky = if let Some(c) = hdri.sample_equirectangular(cc_r) {
                    c * hdri.exposure
                } else {
                    hdri.evaluate_specular(cc_r, 0.08)
                };
                let v_dot_cch = v_dir.dot(cc_h).max(0.0);
                let f_cc = 0.04 + (1.0 - 0.04) * (1.0 - v_dot_cch).powf(5.0);
                radiance += throughput * cc_sky * f_cc * clearcoat;
            }
        }

        // If at max bounces, terminate
        if bounce == max_bounces {
            break;
        }

        // 6. Monte Carlo Indirect Diffuse Bounce (Color Bleeding & Mesh Self-Occlusion)
        let bounce_dir = sample_cosine_hemisphere(normal, rng);
        throughput = throughput * (Vec3::ONE - f0) * (1.0 - metallic) * albedo * ao;
        ray = Ray::new(hit_pos, bounce_dir);

        // Russian Roulette termination for bounces >= 2
        if bounce >= 2 {
            let max_tp = throughput.max_element().clamp(0.05, 0.95);
            if rng.next_f32() > max_tp {
                break;
            }
            throughput /= max_tp;
        }
    }

    radiance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hdri::HdriPreset;

    #[test]
    fn test_xorshift64_prng_distribution() {
        let mut rng = XorShift64::new(42);
        for _ in 0..1000 {
            let val = rng.next_f32();
            assert!(val >= 0.0 && val <= 1.0);
        }
    }

    #[test]
    fn test_sample_cosine_hemisphere_normals() {
        let mut rng = XorShift64::new(12345);
        let n = Vec3::Y;
        for _ in 0..100 {
            let sample = sample_cosine_hemisphere(n, &mut rng);
            assert!(sample.dot(n) >= -0.01);
            assert!((sample.length() - 1.0).abs() < 0.05);
        }
    }

    #[test]
    fn test_sample_ggx_distribution_roughness() {
        let mut rng = XorShift64::new(999);
        let n = Vec3::Y;

        // Smooth mirror surface (roughness = 0.01) must stay extremely close to normal
        for _ in 0..50 {
            let h = sample_ggx(n, 0.01, &mut rng);
            assert!(h.dot(n) > 0.99);
        }

        // Rough surface (roughness = 0.9) must spread across the hemisphere
        let mut spread_count = 0;
        for _ in 0..100 {
            let h = sample_ggx(n, 0.9, &mut rng);
            if h.dot(n) < 0.8 {
                spread_count += 1;
            }
        }
        assert!(spread_count > 10);
    }

    #[test]
    fn test_tonemap_aces_monotonic() {
        let low = tonemap_aces(Vec3::splat(0.2));
        let med = tonemap_aces(Vec3::splat(1.0));
        let high = tonemap_aces(Vec3::splat(5.0));

        assert!(low.x < med.x);
        assert!(med.x < high.x);
        assert!(high.x <= 1.0);
    }

    #[test]
    fn test_path_tracer_accumulation_and_reset() {
        let mut pt = PathTracer::new(64, 64);
        assert_eq!(pt.sample_count, 0);
        assert_eq!(pt.accumulation_buffer.len(), 64 * 64);
        assert_eq!(pt.color_buffer.len(), 64 * 64 * 4);

        let camera = Camera::default();
        let mesh = Mesh::create_cube_mesh("Cube", 1.0);
        let textures = CompositeTextureSet::new(128, 128);
        let hdri = HdriEnvironment::new(HdriPreset::StudioNeutral);

        // Render 2 samples
        pt.render_samples(&camera, &mesh, &textures, &hdri, 2);
        assert_eq!(pt.sample_count, 2);
        assert!(pt.progress_pct() > 0.0);

        // Verify color buffer received non-zero RGBA values
        let non_zero_pixels = pt.color_buffer.chunks_exact(4).filter(|p| p[3] == 255).count();
        assert_eq!(non_zero_pixels, 64 * 64);

        // Test reset
        pt.reset();
        assert_eq!(pt.sample_count, 0);
        assert!(pt.accumulation_buffer.iter().all(|v| *v == Vec3::ZERO));
    }

    #[test]
    fn test_path_tracer_roughness_produces_distinct_highlights() {
        let camera = Camera::default();
        let mesh = Mesh::create_cube_mesh("Cube", 1.0);
        let hdri = HdriEnvironment::new(HdriPreset::StudioNeutral);

        let mut tex_smooth = CompositeTextureSet::new(64, 64);
        tex_smooth.roughness.data.fill(0.02);

        let mut tex_rough = CompositeTextureSet::new(64, 64);
        tex_rough.roughness.data.fill(0.95);

        let mut pt_smooth = PathTracer::new(32, 32);
        pt_smooth.render_samples(&camera, &mesh, &tex_smooth, &hdri, 4);

        let mut pt_rough = PathTracer::new(32, 32);
        pt_rough.render_samples(&camera, &mesh, &tex_rough, &hdri, 4);

        // Calculate differences between the smooth mirror and rough matte renders
        let diff: i32 = pt_smooth.color_buffer
            .iter()
            .zip(pt_rough.color_buffer.iter())
            .map(|(a, b)| (*a as i32 - *b as i32).abs())
            .sum();

        assert!(diff > 500, "Smooth and rough materials must produce visibly distinct path-traced buffers (diff: {})", diff);
    }
}
