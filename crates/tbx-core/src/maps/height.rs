use rayon::prelude::*;
use crate::buffer::{gaussian_blur, GrayF32};
use super::params::HeightParams;
pub fn generate(source: &GrayF32, p: &HeightParams) -> GrayF32 {
    let mut curved = source.clone();
    curved
        .data_mut()
        .par_iter_mut()
        .for_each(|v| *v = ((*v - 0.5) * p.contrast + 0.5 + p.brightness).clamp(0.0, 1.0));
    let blurred = gaussian_blur(&curved, p.blur_radius);
    if p.invert {
        let mut out = blurred;
        out.data_mut().par_iter_mut().for_each(|v| *v = 1.0 - *v);
        out
    } else {
        blurred
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use super::super::params::{HeightSource};
    #[test]
    fn identity_params_preserve_values() {
        let mut src = GrayF32::new(32, 32);
        for v in src.data_mut() {
            *v = 0.5;
        }
        let out = generate(&src, &HeightParams {
            source: HeightSource::Luminance,
            contrast: 1.0, brightness: 0.0, blur_radius: 0.0, invert: false,
        });
        assert!(out.as_slice().iter().all(|v| (v - 0.5).abs() < 1e-4));
    }
    #[test]
    fn invert_flips_values() {
        let mut src = GrayF32::new(8, 8);
        for v in src.data_mut() {
            *v = 0.25;
        }
        let out = generate(&src, &HeightParams {
            source: HeightSource::Luminance,
            contrast: 1.0, brightness: 0.0, blur_radius: 0.0, invert: true,
        });
        assert!(out.as_slice().iter().all(|v| (v - 0.75).abs() < 1e-4));
    }
    #[test]
    fn contrast_amplifies_deviation_from_midpoint() {
        let mut src = GrayF32::new(8, 8);
        for v in src.data_mut() { *v = 0.6; }
        let out = generate(&src, &HeightParams {
            source: HeightSource::Luminance,
            contrast: 2.0, brightness: 0.0, blur_radius: 0.0, invert: false,
        });
        assert!(out.as_slice().iter().all(|v| (v - 0.7).abs() < 1e-4),
            "got {:.4}", out.as_slice()[0]);
    }
}
