use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use crate::buffer::{gaussian_blur, GrayF32};
use crate::color::to_u8;
use super::params::{NormalKernel, NormalParams};
fn detail_enhance(height: &GrayF32) -> GrayF32 {
    let blurred = gaussian_blur(height, 3.0);
    let w = height.width;
    let h = height.height;
    let mut out = GrayF32::new(w, h);
    let amount = 1.5f32;
    out.data_mut()
        .iter_mut()
        .zip(height.as_slice().iter().zip(blurred.as_slice().iter()))
        .for_each(|(dst, (orig, blur))| {
            *dst = (orig + amount * (orig - blur)).clamp(0.0, 1.0);
        });
    out
}
pub fn generate(height: &GrayF32, p: &NormalParams) -> RgbaImage {
    let w = height.width;
    let h = height.height;
    let mut img = RgbaImage::new(w, h);
    let enhanced;
    let height = if p.detail_enhance {
        enhanced = detail_enhance(height);
        &enhanced
    } else {
        height
    };
    let effective_strength = if p.auto_strength {
        let n = (w as f64) * (h as f64);
        let mean = height.as_slice().iter().map(|&v| v as f64).sum::<f64>() / n;
        let variance = height.as_slice().iter()
            .map(|&v| { let d = v as f64 - mean; d * d })
            .sum::<f64>() / n;
        let std_dev = variance.sqrt() as f32;
        let auto = (0.15 / std_dev.max(0.01)).clamp(0.3, 4.0);
        p.strength * auto
    } else {
        p.strength
    };
    let out_rows: Vec<Vec<Rgba<u8>>> = (0..h as i64)
        .into_par_iter()
        .map(|y| {
            (0..w as i64)
                .map(|x| {
                    let (gx, gy) = gradient(height, x, y, p.kernel, p.wrap_edges);
                    let nx = -gx * effective_strength;
                    let ny = -gy * effective_strength;
                    let nz = 1.0f32;
                    let inv = 1.0 / (nx * nx + ny * ny + nz * nz).sqrt();
                    Rgba([
                        to_u8(nx * inv * 0.5 + 0.5),
                        to_u8(ny * inv * 0.5 + 0.5),
                        to_u8(nz * inv * 0.5 + 0.5),
                        255,
                    ])
                })
                .collect()
        })
        .collect();
    for (y, row) in out_rows.iter().enumerate() {
        for (x, px) in row.iter().enumerate() {
            img.put_pixel(x as u32, y as u32, *px);
        }
    }
    img
}
fn gradient(height: &GrayF32, x: i64, y: i64, kernel: NormalKernel, wrap: bool) -> (f32, f32) {
    let s = |dx: i64, dy: i64| -> f32 {
        if wrap {
            height.wrapped(x + dx, y + dy)
        } else {
            height.clamped(x + dx, y + dy)
        }
    };
    match kernel {
        NormalKernel::K3 => {
            let tl = s(-1, -1) * 3.0;
            let l = s(-1, 0) * 10.0;
            let bl = s(-1, 1) * 3.0;
            let tr = s(1, -1) * 3.0;
            let r = s(1, 0) * 10.0;
            let br = s(1, 1) * 3.0;
            let gx = (tr + r + br - tl - l - bl) / 32.0;
            let tt = s(-1, -1) * 3.0;
            let t = s(0, -1) * 10.0;
            let t2 = s(1, -1) * 3.0;
            let bb = s(-1, 1) * 3.0;
            let b = s(0, 1) * 10.0;
            let b2 = s(1, 1) * 3.0;
            let gy = (bb + b + b2 - tt - t - t2) / 32.0;
            (gx, gy)
        }
        NormalKernel::K5 => {
            let weights = [1.0, 2.0, 3.0, 2.0, 1.0];
            let diffs = [-2.0, -1.0, 0.0, 1.0, 2.0];
            let mut gx = 0.0f32;
            let mut gy = 0.0f32;
            for (j, wj) in weights.iter().enumerate() {
                for (i, di) in diffs.iter().enumerate() {
                    let v = s(i as i64 - 2, j as i64 - 2);
                    gx += v * di * wj;
                }
            }
            for (j, dj) in diffs.iter().enumerate() {
                for (i, wi) in weights.iter().enumerate() {
                    let v = s(i as i64 - 2, j as i64 - 2);
                    gy += v * dj * wi;
                }
            }
            let norm = 1.0 / (weights.iter().sum::<f32>() * diffs.iter().map(|d| d.abs()).sum::<f32>());
            (gx * norm, gy * norm)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_height_gives_up_normal() {
        let mut h = GrayF32::new(16, 16);
        for v in h.data_mut() {
            *v = 0.5;
        }
        let img = generate(&h, &NormalParams::default());
        let px = img.get_pixel(8, 8);
        assert!(px[0] == 128 || px[0] == 127);
        assert!(px[1] == 128 || px[1] == 127);
        assert_eq!(px[2], 255);
    }
    #[test]
    fn ramp_tilts_normal() {
        let w = 32u32;
        let h = 32u32;
        let mut height = GrayF32::new(w, h);
        for y in 0..h {
            for x in 0..w {
                height.set(x, y, x as f32 / w as f32);
            }
        }
        let params = NormalParams { auto_strength: false, ..NormalParams::default() };
        let img = generate(&height, &params);
        let px = img.get_pixel(16, 16);
        assert!(px[0] < 128, "rising right -> normal leans left -> R < 128");
        assert!(px[1] >= 120 && px[1] <= 136, "flat vertically -> G near 128");
    }
    #[test]
    fn auto_strength_boosts_flat_source() {
        let w = 32u32; let h = 32u32;
        let mut height = GrayF32::new(w, h);
        for y in 0..h {
            for x in 0..w {
                height.set(x, y, 0.48 + (x as f32 / w as f32) * 0.04);
            }
        }
        let auto_params  = NormalParams { auto_strength: true,  strength: 1.5, ..NormalParams::default() };
        let fixed_params = NormalParams { auto_strength: false, strength: 1.5, ..NormalParams::default() };
        let img_auto  = generate(&height, &auto_params);
        let img_fixed = generate(&height, &fixed_params);
        let r_auto  = img_auto.get_pixel(16, 16)[0] as i32;
        let r_fixed = img_fixed.get_pixel(16, 16)[0] as i32;
        let dev_auto  = (r_auto  - 128).abs();
        let dev_fixed = (r_fixed - 128).abs();
        assert!(dev_auto > dev_fixed,
            "auto_strength should produce more tilt on flat source: auto={dev_auto} fixed={dev_fixed}");
    }
}
