use rayon::prelude::*;
use crate::buffer::{moving_mean, GrayF32};
use super::params::AoParams;
fn occlusion_at_radius(height: &GrayF32, radius: f32, intensity: f32) -> GrayF32 {
    let base = moving_mean(height, radius.max(1.0));
    let mut out = GrayF32::new(height.width, height.height);
    out.data_mut()
        .par_iter_mut()
        .zip(base.as_slice().par_iter().zip(height.as_slice().par_iter()))
        .for_each(|(dst, (b, h))| {
            let cavity = (b - h).clamp(0.0, 1.0);
            *dst = (cavity * intensity * 2.0).clamp(0.0, 1.0);
        });
    out
}
pub fn generate(height: &GrayF32, p: &AoParams) -> GrayF32 {
    let r = p.radius.max(2.0);
    let scale_fine  = occlusion_at_radius(height, (r * 0.25).max(1.0), p.intensity);
    let scale_mid   = occlusion_at_radius(height,  r,                   p.intensity);
    let scale_macro = occlusion_at_radius(height, (r * 3.0).min(200.0), p.intensity * 0.6);
    let base_lp = moving_mean(height, (r * 3.0).min(200.0));
    let mut out = GrayF32::new(height.width, height.height);
    out.data_mut()
        .par_iter_mut()
        .zip(
            scale_fine.as_slice().par_iter()
                .zip(scale_mid.as_slice().par_iter())
                .zip(scale_macro.as_slice().par_iter())
                .zip(base_lp.as_slice().par_iter()),
        )
        .for_each(|(dst, (((fine, mid), mac), b))| {
            let combined_occ = (fine * mid * mac).cbrt();
            let macro_shade = 0.88 + 0.12 * b;
            *dst = ((1.0 - combined_occ) * macro_shade).clamp(0.0, 1.0);
        });
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_surface_has_no_occlusion() {
        let mut h = GrayF32::new(64, 64);
        for v in h.data_mut() {
            *v = 0.5;
        }
        let out = generate(&h, &AoParams::default());
        assert!(out.as_slice().iter().all(|v| *v > 0.80),
            "flat surface should produce near-white AO, got min={:.3}",
            out.as_slice().iter().cloned().fold(f32::INFINITY, f32::min));
    }
    #[test]
    fn cavity_is_darker() {
        let mut h = GrayF32::new(64, 64);
        for v in h.data_mut() {
            *v = 0.8;
        }
        for y in 24..40u32 {
            for x in 24..40u32 {
                h.set(x, y, 0.0);
            }
        }
        let out = generate(&h, &AoParams { radius: 6.0, intensity: 1.0 });
        let pit_avg: f32 = (28..36u32)
            .flat_map(|y| (28..36u32).map(move |x| (x, y)))
            .map(|(x, y)| out.at(x, y))
            .sum::<f32>() / 64.0;
        let flat_avg: f32 = (0..8u32)
            .flat_map(|y| (0..8u32).map(move |x| (x, y)))
            .map(|(x, y)| out.at(x, y))
            .sum::<f32>() / 64.0;
        assert!(pit_avg < flat_avg,
            "cavity (avg={:.3}) should be darker than flat (avg={:.3})", pit_avg, flat_avg);
    }
    #[test]
    fn deeper_cavity_is_darker_than_shallow() {
        let mut h = GrayF32::new(64, 64);
        for v in h.data_mut() { *v = 0.8; }
        for y in 10..22u32 { for x in 2..14u32 { h.set(x, y, 0.5); } }
        for y in 10..22u32 { for x in 34..46u32 { h.set(x, y, 0.0); } }
        let out = generate(&h, &AoParams { radius: 4.0, intensity: 1.0 });
        let shallow: f32 = (12..20u32).flat_map(|y| (4..12u32).map(move |x| (x, y)))
            .map(|(x, y)| out.at(x, y)).sum::<f32>() / 64.0;
        let deep: f32 = (12..20u32).flat_map(|y| (36..44u32).map(move |x| (x, y)))
            .map(|(x, y)| out.at(x, y)).sum::<f32>() / 64.0;
        assert!(deep < shallow,
            "deep cavity (avg={:.3}) should be darker than shallow (avg={:.3})", deep, shallow);
    }
}
