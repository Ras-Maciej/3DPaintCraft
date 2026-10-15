// 3D Paintcraft - Math & Noise Utilities
// Built with pure Rust for ArtCraft ecosystem

use glam::Vec4;

/// Fast Perlin / Simplex style 2D and 3D procedural noise
pub struct NoiseGenerator;

impl NoiseGenerator {
    /// 2D Fractional Brownian Motion (fBm) for organic surfaces (rust, dirt, scratches)
    pub fn fbm_2d(x: f32, y: f32, octaves: usize, lacunarity: f32, gain: f32) -> f32 {
        let mut total = 0.0;
        let mut frequency = 1.0;
        let mut amplitude = 1.0;
        let mut max_val = 0.0;

        for _ in 0..octaves {
            total += Self::value_noise_2d(x * frequency, y * frequency) * amplitude;
            max_val += amplitude;
            frequency *= lacunarity;
            amplitude *= gain;
        }

        if max_val > 0.0 {
            total / max_val
        } else {
            0.0
        }
    }

    /// Fast smooth 2D value noise
    pub fn value_noise_2d(x: f32, y: f32) -> f32 {
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let fx = x - x.floor();
        let fy = y - y.floor();

        // Smoothstep interpolation (Hermite curve)
        let sx = fx * fx * (3.0 - 2.0 * fx);
        let sy = fy * fy * (3.0 - 2.0 * fy);

        let n00 = Self::hash_2d(ix, iy);
        let n10 = Self::hash_2d(ix + 1, iy);
        let n01 = Self::hash_2d(ix, iy + 1);
        let n11 = Self::hash_2d(ix + 1, iy + 1);

        let nx0 = n00 * (1.0 - sx) + n10 * sx;
        let nx1 = n01 * (1.0 - sx) + n11 * sx;

        nx0 * (1.0 - sy) + nx1 * sy
    }

    /// Hash coordinate to pseudo-random float [0.0, 1.0]
    #[inline]
    fn hash_2d(x: i32, y: i32) -> f32 {
        let mut n = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263);
        n = (n ^ (n >> 13)).wrapping_mul(1274126177);
        ((n & 0x7fffffff) as f32) / (0x7fffffff as f32)
    }

    /// Cellular / Worley noise for scuffs, flakes, and scratches
    pub fn voronoi_2d(x: f32, y: f32) -> (f32, f32) {
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;

        let mut min_dist = 100.0;
        let mut second_dist = 100.0;

        for dx in -1..=1 {
            for dy in -1..=1 {
                let cell_x = ix + dx;
                let cell_y = iy + dy;

                let r_x = Self::hash_2d(cell_x, cell_y);
                let r_y = Self::hash_2d(cell_x + 91, cell_y + 137);

                let point_x = cell_x as f32 + r_x;
                let point_y = cell_y as f32 + r_y;

                let dist = ((x - point_x).powi(2) + (y - point_y).powi(2)).sqrt();

                if dist < min_dist {
                    second_dist = min_dist;
                    min_dist = dist;
                } else if dist < second_dist {
                    second_dist = dist;
                }
            }
        }

        (min_dist, second_dist)
    }

    /// 2D Perlin Gradient Noise with smooth quintic Hermite interpolation returning [-1.0, 1.0]
    pub fn perlin_2d(x: f32, y: f32) -> f32 {
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let fx = x - x.floor();
        let fy = y - y.floor();

        // Quintic Hermite curve: 6t^5 - 15t^4 + 10t^3 (zero 1st & 2nd derivatives at knots)
        let u = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0);
        let v = fy * fy * fy * (fy * (fy * 6.0 - 15.0) + 10.0);

        const GRADIENTS: [[f32; 2]; 8] = [
            [1.0, 0.0],
            [-1.0, 0.0],
            [0.0, 1.0],
            [0.0, -1.0],
            [0.70710677, 0.70710677],
            [-0.70710677, 0.70710677],
            [0.70710677, -0.70710677],
            [-0.70710677, -0.70710677],
        ];

        let g00 = GRADIENTS[(Self::hash_index(ix, iy) & 7) as usize];
        let g10 = GRADIENTS[(Self::hash_index(ix + 1, iy) & 7) as usize];
        let g01 = GRADIENTS[(Self::hash_index(ix, iy + 1) & 7) as usize];
        let g11 = GRADIENTS[(Self::hash_index(ix + 1, iy + 1) & 7) as usize];

        let d00 = g00[0] * fx + g00[1] * fy;
        let d10 = g10[0] * (fx - 1.0) + g10[1] * fy;
        let d01 = g01[0] * fx + g01[1] * (fy - 1.0);
        let d11 = g11[0] * (fx - 1.0) + g11[1] * (fy - 1.0);

        let nx0 = d00 * (1.0 - u) + d10 * u;
        let nx1 = d01 * (1.0 - u) + d11 * u;

        nx0 * (1.0 - v) + nx1 * v
    }

    #[inline]
    fn hash_index(x: i32, y: i32) -> u32 {
        let mut n = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
        n = (n ^ (n >> 13)).wrapping_mul(1274126177);
        n ^ (n >> 16)
    }

    /// Normalized Perlin gradient noise remapped to [0.0, 1.0]
    #[allow(dead_code)]
    pub fn perlin_2d_norm(x: f32, y: f32) -> f32 {
        (Self::perlin_2d(x, y) * 0.7071 + 0.5).clamp(0.0, 1.0)
    }

    /// Multi-octave Perlin Gradient FBM [0.0, 1.0]
    pub fn fbm_perlin_2d(x: f32, y: f32, octaves: usize, lacunarity: f32, gain: f32) -> f32 {
        let mut total = 0.0;
        let mut freq = 1.0;
        let mut amp = 1.0;
        let mut max_val = 0.0;

        for _ in 0..octaves {
            total += Self::perlin_2d(x * freq, y * freq) * amp;
            max_val += amp;
            freq *= lacunarity;
            amp *= gain;
        }

        if max_val > 0.0 {
            ((total / max_val) * 0.7071 + 0.5).clamp(0.0, 1.0)
        } else {
            0.5
        }
    }

    /// Billow Noise: puffy rolling cloud-like and smoke volume shapes [0.0, 1.0]
    pub fn billow_2d(x: f32, y: f32, octaves: usize, lacunarity: f32, gain: f32) -> f32 {
        let mut total = 0.0;
        let mut freq = 1.0;
        let mut amp = 1.0;
        let mut max_val = 0.0;

        for _ in 0..octaves {
            let n = Self::perlin_2d(x * freq, y * freq).abs() * 2.0 - 1.0;
            total += n * amp;
            max_val += amp;
            freq *= lacunarity;
            amp *= gain;
        }

        if max_val > 0.0 {
            ((total / max_val) * 0.7071 + 0.5).clamp(0.0, 1.0)
        } else {
            0.5
        }
    }

    /// Ridged Multifractal: sharp jagged mountain ridges, cracked earth, stone fissures [0.0, 1.0]
    pub fn ridged_fbm_2d(x: f32, y: f32, octaves: usize, lacunarity: f32, gain: f32) -> f32 {
        let mut total = 0.0;
        let mut freq = 1.0;
        let mut amp = 1.0;
        let mut max_val = 0.0;

        for _ in 0..octaves {
            let n = 1.0 - Self::perlin_2d(x * freq, y * freq).abs();
            total += n * n * amp;
            max_val += amp;
            freq *= lacunarity;
            amp *= gain;
        }

        if max_val > 0.0 {
            (total / max_val).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Inigo Quilez Domain-Warped Turbulent Grunge: organic marbled fluid weathering [0.0, 1.0]
    pub fn domain_warp_2d(x: f32, y: f32) -> f32 {
        let qx = Self::fbm_2d(x, y, 3, 2.0, 0.5);
        let qy = Self::fbm_2d(x + 5.2, y + 1.3, 3, 2.0, 0.5);

        let rx = Self::fbm_2d(x + 4.0 * qx + 1.7, y + 4.0 * qy + 9.2, 3, 2.0, 0.5);
        let ry = Self::fbm_2d(x + 4.0 * qx + 8.3, y + 4.0 * qy + 2.8, 3, 2.0, 0.5);

        Self::fbm_2d(x + 4.0 * rx, y + 4.0 * ry, 4, 2.0, 0.5)
    }

    /// Poisson-jittered splatter spots, rust pitting, and liquid droplets [0.0, 1.0]
    pub fn splatter_spots_2d(x: f32, y: f32) -> f32 {
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let mut max_spot = 0.0f32;

        for dx in -1..=1 {
            for dy in -1..=1 {
                let cx = ix + dx;
                let cy = iy + dy;
                let h1 = Self::hash_2d(cx, cy);
                let h2 = Self::hash_2d(cx + 43, cy + 89);
                let h3 = Self::hash_2d(cx + 17, cy + 31);

                if h1 > 0.35 {
                    let spot_x = cx as f32 + h2 * 0.8 + 0.1;
                    let spot_y = cy as f32 + h3 * 0.8 + 0.1;
                    let spot_radius = 0.08 + (h1 - 0.35) * 0.45;

                    let d = ((x - spot_x).powi(2) + (y - spot_y).powi(2)).sqrt();
                    if d < spot_radius {
                        let falloff = 1.0 - (d / spot_radius).powi(2);
                        if falloff > max_spot {
                            max_spot = falloff;
                        }
                    }
                }
            }
        }
        max_spot.clamp(0.0, 1.0)
    }

    /// Wood grain with annual growth rings and longitudinal anisotropic fibers [0.0, 1.0]
    pub fn wood_grain_2d(x: f32, y: f32) -> f32 {
        let fiber = Self::fbm_2d(x * 16.0, y * 0.5, 3, 2.0, 0.5);
        let wobble = Self::fbm_2d(x * 0.8, y * 0.2, 2, 2.0, 0.5);
        let dist = (x * 0.3 + wobble * 0.5).hypot(y * 0.05 + wobble * 0.2);
        let rings = ((dist * 30.0 + fiber * 2.0).sin() * 0.5 + 0.5).powf(0.8);
        (rings * 0.75 + fiber * 0.25).clamp(0.0, 1.0)
    }

    /// Interlaced textile fabric / carbon fiber weave pattern with yarn micro-texture [0.0, 1.0]
    pub fn fabric_weave_2d(x: f32, y: f32) -> f32 {
        let pi = std::f32::consts::PI;
        let thread_x = (x * pi * 4.0).sin();
        let thread_y = (y * pi * 4.0).sin();
        let checker = ((x * 2.0).floor() as i32 + (y * 2.0).floor() as i32) % 2 == 0;
        let weave = if checker {
            thread_x * 0.5 + 0.5
        } else {
            thread_y * 0.5 + 0.5
        };
        let micro = Self::value_noise_2d(x * 32.0, y * 32.0) * 0.15;
        (weave * 0.85 + micro).clamp(0.0, 1.0)
    }
}

/// Convert sRGB [0.0, 1.0] to Linear RGB [0.0, 1.0]
#[allow(dead_code)]
#[inline]
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert Linear RGB [0.0, 1.0] to sRGB [0.0, 1.0]
#[allow(dead_code)]
#[inline]
pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Clamps and converts [r, g, b, a] floats [0..1] to [u8; 4]
#[allow(dead_code)]
#[inline]
pub fn vec4_to_rgba_u8(v: Vec4) -> [u8; 4] {
    [
        (v.x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        (v.y.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        (v.z.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        (v.w.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
    ]
}

/// Converts [u8; 4] to Vec4 [0.0, 1.0]
#[allow(dead_code)]
#[inline]
pub fn rgba_u8_to_vec4(b: [u8; 4]) -> Vec4 {
    Vec4::new(
        b[0] as f32 / 255.0,
        b[1] as f32 / 255.0,
        b[2] as f32 / 255.0,
        b[3] as f32 / 255.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_algorithms_bounds_and_stability() {
        for y_step in 0..16 {
            for x_step in 0..16 {
                let x = x_step as f32 * 0.35 + 0.12;
                let y = y_step as f32 * 0.35 + 0.12;

                let p = NoiseGenerator::perlin_2d(x, y);
                assert!(p >= -1.05 && p <= 1.05, "Perlin noise should be bounded [-1, 1], got {}", p);

                let fbm = NoiseGenerator::fbm_perlin_2d(x, y, 4, 2.0, 0.5);
                assert!((0.0..=1.0).contains(&fbm));

                let billow = NoiseGenerator::billow_2d(x, y, 3, 2.0, 0.5);
                assert!((0.0..=1.0).contains(&billow));

                let ridged = NoiseGenerator::ridged_fbm_2d(x, y, 3, 2.0, 0.5);
                assert!((0.0..=1.0).contains(&ridged));

                let warp = NoiseGenerator::domain_warp_2d(x, y);
                assert!((0.0..=1.0).contains(&warp));

                let splatter = NoiseGenerator::splatter_spots_2d(x, y);
                assert!((0.0..=1.0).contains(&splatter));

                let wood = NoiseGenerator::wood_grain_2d(x, y);
                assert!((0.0..=1.0).contains(&wood));

                let weave = NoiseGenerator::fabric_weave_2d(x, y);
                assert!((0.0..=1.0).contains(&weave));
            }
        }
    }
}

