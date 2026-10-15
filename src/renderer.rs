// 3D Paintcraft - 3D PBR Viewport Renderer, Camera & HDRI Lighting
// Pure safe Rust with perspective-correct interpolation and image-based lighting

use crate::hdri::{HdriEnvironment, HdriPreset};
use crate::layer::CompositeTextureSet;
use crate::mesh::{Mesh, Ray};
use glam::{Mat4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewportChannelMode {
    Material,         // Complete Lit PBR
    BaseColor,        // Unlit Albedo
    Roughness,        // Grayscale Roughness
    Metallic,         // Grayscale Metallic
    Normal,           // Normal Map RGB
    Height,           // Height / Bump
    AmbientOcclusion, // Ambient Occlusion
    Curvature,        // Curvature Map
    Position,         // World Position Object (WPO)
    WorldNormal,      // World Space Normal
    Thickness,        // Thickness Map
}

impl ViewportChannelMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Material => "Lit Material (PBR)",
            Self::BaseColor => "Base Color",
            Self::Roughness => "Roughness",
            Self::Metallic => "Metallic",
            Self::Normal => "Normal Map",
            Self::Height => "Height / Bump",
            Self::AmbientOcclusion => "Ambient Occlusion (AO)",
            Self::Curvature => "Curvature",
            Self::Position => "Position (WPO)",
            Self::WorldNormal => "World Space Normal",
            Self::Thickness => "Thickness",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Camera {
    pub target: Vec3,
    pub yaw: f32,       // Radians
    pub pitch: f32,     // Radians
    pub distance: f32,
    pub fov_y: f32,     // Radians
    pub is_orthographic: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: 0.8,
            pitch: 0.35,
            distance: 2.8,
            fov_y: 50.0f32.to_radians(),
            is_orthographic: false,
        }
    }
}

impl Camera {
    pub fn eye_position(&self) -> Vec3 {
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();

        let offset = Vec3::new(cp * sy, sp, cp * cy) * self.distance;
        self.target + offset
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye_position(), self.target, Vec3::Y)
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        if self.is_orthographic {
            let half_h = self.distance * 0.45;
            let half_w = half_h * aspect;
            Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, 0.1, 100.0)
        } else {
            Mat4::perspective_rh(self.fov_y, aspect, 0.1, 100.0)
        }
    }

    /// Screen pixel to world space Ray for accurate projection painting
    /// Project a 3D world space coordinate into 2D viewport screen coordinates
    pub fn world_to_screen(&self, world_pos: Vec3, width: f32, height: f32) -> Option<Vec2> {
        let aspect = width / height.max(1.0);
        let proj = self.projection_matrix(aspect);
        let view = self.view_matrix();
        let vp = proj * view;

        let clip = vp.project_point3(world_pos);
        if clip.z < 0.0 || clip.z > 1.0 {
            return None;
        }

        let screen_x = (clip.x * 0.5 + 0.5) * width;
        let screen_y = (1.0 - (clip.y * 0.5 + 0.5)) * height;
        Some(Vec2::new(screen_x, screen_y))
    }

    /// Screen pixel to world space Ray for accurate projection painting
    pub fn screen_to_ray(&self, screen_x: f32, screen_y: f32, width: f32, height: f32) -> Ray {
        let ndc_x = (2.0 * screen_x / width) - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_y / height);

        let aspect = width / height.max(1.0);
        let proj = self.projection_matrix(aspect);
        let view = self.view_matrix();
        let inv_view_proj = (proj * view).inverse();

        if self.is_orthographic {
            let near_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
            let far_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
            Ray::new(near_point, (far_point - near_point).normalize())
        } else {
            let eye = self.eye_position();
            let near_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
            Ray::new(eye, (near_point - eye).normalize())
        }
    }

    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw -= delta_x * 0.008;
        self.pitch = (self.pitch + delta_y * 0.008).clamp(-1.5, 1.5);
    }

    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let eye = self.eye_position();
        let forward = (self.target - eye).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();

        let pan_speed = self.distance * 0.0015;
        self.target -= right * delta_x * pan_speed;
        self.target += up * delta_y * pan_speed;
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance * (1.0 - delta * 0.1)).clamp(0.5, 20.0);
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub struct ViewportRenderer {
    pub width: u32,
    pub height: u32,
    pub depth_buffer: Vec<f32>,
    pub color_buffer: Vec<u8>,
    pub hdri: HdriEnvironment,
    pub show_wireframe: bool,
    #[allow(dead_code)]
    pub show_grid: bool,
}

impl ViewportRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        let pixel_count = (width * height) as usize;
        Self {
            width,
            height,
            depth_buffer: vec![f32::MAX; pixel_count],
            color_buffer: vec![0; pixel_count * 4],
            hdri: HdriEnvironment::new(HdriPreset::StudioNeutral),
            show_wireframe: false,
            show_grid: true,
        }
    }

    pub fn resize(&mut self, new_w: u32, new_h: u32) {
        if self.width != new_w || self.height != new_h {
            self.width = new_w.max(64);
            self.height = new_h.max(64);
            let pixel_count = (self.width * self.height) as usize;
            self.depth_buffer.resize(pixel_count, f32::MAX);
            self.color_buffer.resize(pixel_count * 4, 0);
        }
    }

    /// Render 3D viewport with perspective-correct interpolation, TBN normal mapping, and HDRI IBL
    pub fn render(
        &mut self,
        camera: &Camera,
        mesh: &Mesh,
        textures: &CompositeTextureSet,
        mode: ViewportChannelMode,
    ) {
        let w = self.width;
        let h = self.height;

        // Clear color (Substance Painter dark vignette background)
        for y in 0..h {
            let t = y as f32 / h as f32;
            let bg_r = (22.0 + t * 6.0) as u8;
            let bg_g = (23.0 + t * 6.0) as u8;
            let bg_b = (26.0 + t * 7.0) as u8;

            for x in 0..w {
                let idx = ((y * w + x) * 4) as usize;
                self.color_buffer[idx] = bg_r;
                self.color_buffer[idx + 1] = bg_g;
                self.color_buffer[idx + 2] = bg_b;
                self.color_buffer[idx + 3] = 255;
            }
        }

        // Clear depth
        self.depth_buffer.fill(f32::MAX);

        let aspect = w as f32 / h as f32;
        let view = camera.view_matrix();
        let proj = camera.projection_matrix(aspect);
        let vp = proj * view;
        let eye = camera.eye_position();

        let tri_count = mesh.indices.len() / 3;

        // Rasterize triangles
        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let v0 = &mesh.vertices[i0];
            let v1 = &mesh.vertices[i1];
            let v2 = &mesh.vertices[i2];

            // Transform to clip space
            let c0 = vp * Vec4::new(v0.position.x, v0.position.y, v0.position.z, 1.0);
            let c1 = vp * Vec4::new(v1.position.x, v1.position.y, v1.position.z, 1.0);
            let c2 = vp * Vec4::new(v2.position.x, v2.position.y, v2.position.z, 1.0);

            // Clip triangle if behind near plane
            if c0.w <= 0.01 || c1.w <= 0.01 || c2.w <= 0.01 {
                continue;
            }

            // Perspective division to NDC
            let ndc0 = c0.truncate() / c0.w;
            let ndc1 = c1.truncate() / c1.w;
            let ndc2 = c2.truncate() / c2.w;

            // Back-face culling
            let cross = (ndc1.x - ndc0.x) * (ndc2.y - ndc0.y) - (ndc1.y - ndc0.y) * (ndc2.x - ndc0.x);
            if cross <= 0.0 {
                continue;
            }

            // Screen pixel coordinates
            let p0 = Vec2::new((ndc0.x * 0.5 + 0.5) * w as f32, (1.0 - (ndc0.y * 0.5 + 0.5)) * h as f32);
            let p1 = Vec2::new((ndc1.x * 0.5 + 0.5) * w as f32, (1.0 - (ndc1.y * 0.5 + 0.5)) * h as f32);
            let p2 = Vec2::new((ndc2.x * 0.5 + 0.5) * w as f32, (1.0 - (ndc2.y * 0.5 + 0.5)) * h as f32);

            let min_x = (p0.x.min(p1.x).min(p2.x).floor() as i32).max(0);
            let max_x = (p0.x.max(p1.x).max(p2.x).ceil() as i32).min(w as i32 - 1);
            let min_y = (p0.y.min(p1.y).min(p2.y).floor() as i32).max(0);
            let max_y = (p0.y.max(p1.y).max(p2.y).ceil() as i32).min(h as i32 - 1);

            let denom = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
            if denom.abs() < 1e-5 {
                continue;
            }

            // Precompute inverse W values for perspective-correct interpolation
            let inv_w0 = 1.0 / c0.w;
            let inv_w1 = 1.0 / c1.w;
            let inv_w2 = 1.0 / c2.w;

            for py in min_y..=max_y {
                for px in min_x..=max_x {
                    let screen_pt = Vec2::new(px as f32 + 0.5, py as f32 + 0.5);

                    // Screen-space barycentrics
                    let w0 = ((p1.y - p2.y) * (screen_pt.x - p2.x) + (p2.x - p1.x) * (screen_pt.y - p2.y)) / denom;
                    let w1 = ((p2.y - p0.y) * (screen_pt.x - p2.x) + (p0.x - p2.x) * (screen_pt.y - p2.y)) / denom;
                    let w2 = 1.0 - w0 - w1;

                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }

                    // Perspective-correct depth
                    let depth = w0 * ndc0.z + w1 * ndc1.z + w2 * ndc2.z;
                    let pixel_idx = (py as u32 * w + px as u32) as usize;

                    if depth >= self.depth_buffer[pixel_idx] {
                        continue;
                    }
                    self.depth_buffer[pixel_idx] = depth;

                    // PERSPECTIVE-CORRECT BARYCENTRICS (Fixes perspective warping!)
                    let pw0 = w0 * inv_w0;
                    let pw1 = w1 * inv_w1;
                    let pw2 = w2 * inv_w2;
                    let inv_sum = 1.0 / (pw0 + pw1 + pw2);
                    let pc0 = pw0 * inv_sum;
                    let pc1 = pw1 * inv_sum;
                    let pc2 = pw2 * inv_sum;

                    // Interpolate vertex attributes with perspective correctness
                    let world_pos = v0.position * pc0 + v1.position * pc1 + v2.position * pc2;
                    let geo_normal = (v0.normal * pc0 + v1.normal * pc1 + v2.normal * pc2).normalize_or_zero();
                    let tangent = (v0.tangent.truncate() * pc0 + v1.tangent.truncate() * pc1 + v2.tangent.truncate() * pc2).normalize_or_zero();
                    let uv = v0.uv * pc0 + v1.uv * pc1 + v2.uv * pc2;

                    // Sample PBR textures
                    let base_col = textures.base_color.sample_bilinear(uv);
                    let roughness = textures.roughness.sample_bilinear(uv).clamp(0.015, 1.0);
                    let metallic = textures.metallic.sample_bilinear(uv).clamp(0.0, 1.0);
                    let normal_sample = textures.normal.sample_bilinear(uv);
                    let ao = textures.ao.sample_bilinear(uv);

                    // Compute TBN tangent-space normal perturbation
                    let bitangent = geo_normal.cross(tangent) * v0.tangent.w;
                    let n_map = normal_sample.truncate() * 2.0 - Vec3::ONE;
                    let perturbed_normal = if n_map.length_squared() > 1e-4 {
                        (tangent * n_map.x + bitangent * n_map.y + geo_normal * n_map.z).normalize_or_zero()
                    } else {
                        geo_normal
                    };

                    // Compute pixel color based on view mode
                    let final_color = match mode {
                        ViewportChannelMode::BaseColor => base_col.truncate(),
                        ViewportChannelMode::Roughness => Vec3::splat(roughness),
                        ViewportChannelMode::Metallic => Vec3::splat(metallic),
                        ViewportChannelMode::Normal => normal_sample.truncate(),
                        ViewportChannelMode::Height => Vec3::splat(textures.height_map.sample_bilinear(uv)),
                        ViewportChannelMode::AmbientOcclusion => Vec3::splat(ao),
                        ViewportChannelMode::Curvature => Vec3::splat(0.5),
                        ViewportChannelMode::Position => (world_pos * 0.5 + Vec3::splat(0.5)).clamp(Vec3::ZERO, Vec3::ONE),
                        ViewportChannelMode::WorldNormal => perturbed_normal * 0.5 + Vec3::splat(0.5),
                        ViewportChannelMode::Thickness => Vec3::splat(ao),
                        ViewportChannelMode::Material => {
                            // Cook-Torrance GGX PBR with selectable HDRI IBL
                            self.evaluate_pbr_hdri(
                                world_pos,
                                perturbed_normal,
                                eye,
                                base_col.truncate(),
                                roughness,
                                metallic,
                                ao,
                            )
                        }
                    };

                    // Wireframe overlay check
                    let is_wire = if self.show_wireframe {
                        let edge_dist = w0.min(w1).min(w2);
                        edge_dist < 0.03
                    } else {
                        false
                    };

                    let color_to_write = if is_wire {
                        Vec3::new(0.0, 0.85, 1.0)
                    } else {
                        final_color
                    };

                    let c_idx = pixel_idx * 4;
                    self.color_buffer[c_idx] = (color_to_write.x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    self.color_buffer[c_idx + 1] = (color_to_write.y.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    self.color_buffer[c_idx + 2] = (color_to_write.z.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    self.color_buffer[c_idx + 3] = 255;
                }
            }
        }
    }

    /// Complete Cook-Torrance GGX PBR with full HDRI Diffuse & Specular IBL
    fn evaluate_pbr_hdri(
        &self,
        world_pos: Vec3,
        normal: Vec3,
        eye: Vec3,
        albedo: Vec3,
        roughness: f32,
        metallic: f32,
        ao: f32,
    ) -> Vec3 {
        let n = normal;
        let v = (eye - world_pos).normalize_or_zero();
        let l = self.hdri.key_light_direction();
        let h = (v + l).normalize_or_zero();

        // Directional Sun lighting with smooth soft shadow wrap
        let n_dot_l_raw = n.dot(l);
        let n_dot_l = n_dot_l_raw.max(0.0);
        let n_dot_v = n.dot(v).max(1e-4);
        let n_dot_h = n.dot(h).max(0.0);
        let v_dot_h = v.dot(h).max(0.0);

        // Dielectric F0 is 0.04, metal F0 is tinted by albedo
        let f0 = Vec3::splat(0.04).lerp(albedo, metallic);

        // Fresnel-Schlick
        let f = f0 + (Vec3::ONE - f0) * (1.0 - v_dot_h).powi(5);

        // GGX Normal Distribution D (accurate, without artificial 1e-5 clamping!)
        let r_clamped = roughness.clamp(0.015, 1.0);
        let alpha = r_clamped * r_clamped;
        let alpha2 = (alpha * alpha).max(1e-8);
        let denom_term = (n_dot_h * n_dot_h) * (alpha2 - 1.0) + 1.0;
        let denom_d = (std::f32::consts::PI * denom_term * denom_term).max(1e-10);
        let d = alpha2 / denom_d;

        // Smith Geometry shadowing G
        let k = (r_clamped + 1.0).powi(2) / 8.0;
        let g_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
        let g_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
        let g = g_l * g_v;

        // Cook-Torrance Specular BRDF
        let specular_brdf = (d * g * f) / (4.0 * n_dot_l * n_dot_v).max(1e-4);

        // Diffuse Lambertian with energy conservation
        let kd = (Vec3::ONE - f) * (1.0 - metallic);
        let diffuse_brdf = kd * albedo / std::f32::consts::PI;

        let dielectric_falloff = (1.0 - r_clamped).clamp(0.0, 1.0).powf(1.5);
        let matte_factor = dielectric_falloff * (1.0 - metallic) + metallic;
        let sun_color = self.hdri.key_light_color();
        let direct_specular = specular_brdf * sun_color * n_dot_l * matte_factor;
        let direct_diffuse = diffuse_brdf * sun_color * n_dot_l;
        let direct_light = direct_diffuse + direct_specular;

        // --- HDRI IMAGE-BASED LIGHTING (IBL) ---
        // 1. Diffuse Irradiance from HDRI
        let diffuse_irradiance = self.hdri.evaluate_diffuse(n);
        let ibl_diffuse = kd * albedo * diffuse_irradiance;

        // 2. Prefiltered Specular Reflection from HDRI
        // Outgoing reflection vector
        let r = (2.0 * n.dot(v) * n - v).normalize_or_zero();
        let specular_radiance = self.hdri.evaluate_specular(r, roughness);

        // Environment Fresnel with roughness attenuation (Lagarde / Frostbite / Substance)
        let gloss = 1.0 - r_clamped;
        let f90 = Vec3::splat(gloss).max(f0);
        let f_env = f0 + (f90 - f0) * (1.0 - n_dot_v).powi(5);

        // Specular reflection: metal reflects full tinted env, dielectric has sharp gloss reflection
        let ibl_specular = specular_radiance * (f_env + Vec3::splat(0.04 * gloss * gloss)) * matte_factor;

        // Ambient contact shadowing
        let contact_occlusion = (n.y * 0.2 + 0.8).clamp(0.2, 1.0) * ao;

        (direct_light + (ibl_diffuse + ibl_specular) * contact_occlusion).clamp(Vec3::ZERO, Vec3::splat(3.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dielectric_roughness_matte() {
        let renderer = ViewportRenderer::new(128, 128);
        let world_pos = Vec3::ZERO;
        let normal = Vec3::Y;
        let eye = Vec3::new(0.0, 1.0, 1.0);
        let black_albedo = Vec3::ZERO;
        let metallic = 0.0;
        let ao = 1.0;

        // Shiny dielectric (roughness = 0.015): specular reflection highlights are present
        let shiny_color = renderer.evaluate_pbr_hdri(
            world_pos, normal, eye, black_albedo, 0.015, metallic, ao,
        );
        assert!(shiny_color.length() > 0.01, "Shiny black dielectric must have visible specular reflections");

        // Completely rough dielectric (roughness = 1.0): specular reflection highlights are completely extinguished
        let matte_color = renderer.evaluate_pbr_hdri(
            world_pos, normal, eye, black_albedo, 1.0, metallic, ao,
        );
        assert_eq!(matte_color, Vec3::ZERO, "Rough dielectric at roughness 1.0 must be completely matte with zero specular reflections");
    }
}
