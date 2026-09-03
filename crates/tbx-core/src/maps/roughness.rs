use rayon::prelude::*;
use crate::buffer::{moving_mean, GrayF32};
use super::params::RoughnessParams;
pub fn generate(height: &GrayF32, p: &RoughnessParams) -> GrayF32 {
    let radius = p.radius.max(1.0);
    let mean = moving_mean(height, radius);
    let mut sq = height.clone();
    sq.data_mut().par_iter_mut().for_each(|v| *v = *v * *v);
    let mean_sq = moving_mean(&sq, radius);
    let mut out = GrayF32::new(height.width, height.height);
    out.data_mut()
        .par_iter_mut()
        .zip(mean.as_slice().par_iter().zip(mean_sq.as_slice().par_iter()))
        .for_each(|(dst, (m, ms))| {
            let var = (ms - m * m).max(0.0);
            let std = var.sqrt();
            let r = (std * p.amount * 4.0).clamp(0.0, 1.0);
            let curved = if (p.curve - 1.0).abs() > 1e-4 { r.powf(p.curve.max(0.01)) } else { r };
            *dst = if p.invert { 1.0 - curved } else { curved };
        });
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_surface_is_smooth() {
        let mut h = GrayF32::new(32, 32);
        for v in h.data_mut() {
            *v = 0.5;
        }
        let out = generate(&h, &RoughnessParams::default());
        assert!(out.as_slice().iter().all(|v| *v < 0.01));
    }
    #[test]
    fn noisy_surface_is_rough() {
        let mut h = GrayF32::new(32, 32);
        for (i, v) in h.data_mut().iter_mut().enumerate() {
            *v = if i % 2 == 0 { 0.0 } else { 1.0 };
        }
        let out = generate(&h, &RoughnessParams::default());
        let avg: f32 = out.as_slice().iter().sum::<f32>() / out.as_slice().len() as f32;
        assert!(avg > 0.5);
    }
}
