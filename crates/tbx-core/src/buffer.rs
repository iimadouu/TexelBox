use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use crate::color::luminance;
#[derive(Clone, Debug)]
pub struct GrayF32 {
    pub width: u32,
    pub height: u32,
    data: Vec<f32>,
}
impl GrayF32 {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height, data: vec![0.0; (width as usize) * (height as usize)] }
    }
    pub fn from_vec(width: u32, height: u32, data: Vec<f32>) -> Self {
        assert_eq!(data.len(), (width as usize) * (height as usize));
        Self { width, height, data }
    }
    #[inline]
    pub fn at(&self, x: u32, y: u32) -> f32 {
        self.data[(y as usize) * (self.width as usize) + (x as usize)]
    }
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, v: f32) {
        self.data[(y as usize) * (self.width as usize) + (x as usize)] = v;
    }
    #[inline]
    pub fn clamped(&self, x: i64, y: i64) -> f32 {
        let cx = x.clamp(0, self.width as i64 - 1) as u32;
        let cy = y.clamp(0, self.height as i64 - 1) as u32;
        self.at(cx, cy)
    }
    #[inline]
    pub fn wrapped(&self, x: i64, y: i64) -> f32 {
        let w = self.width as i64;
        let h = self.height as i64;
        let cx = x.rem_euclid(w) as u32;
        let cy = y.rem_euclid(h) as u32;
        self.at(cx, cy)
    }
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }
    pub fn into_vec(self) -> Vec<f32> {
        self.data
    }
    pub fn data_mut(&mut self) -> &mut [f32] {
        &mut self.data
    }
    pub fn from_rgba(img: &RgbaImage) -> Self {
        let (w, h) = img.dimensions();
        let mut out = Self::new(w, h);
        out.data
            .par_iter_mut()
            .zip(img.as_raw().par_chunks(4))
            .for_each(|(dst, px)| *dst = luminance(px[0], px[1], px[2]));
        out
    }
    pub fn from_rgba_channel(img: &RgbaImage, channel: usize) -> Self {
        let ch = channel.min(3);
        let (w, h) = img.dimensions();
        let mut out = Self::new(w, h);
        out.data
            .par_iter_mut()
            .zip(img.as_raw().par_chunks(4))
            .for_each(|(dst, px)| *dst = px[ch] as f32 / 255.0);
        out
    }
    pub fn to_rgba(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);
        for (dst, src) in img.pixels_mut().zip(self.data.iter()) {
            let v = (src.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            *dst = Rgba([v, v, v, 255]);
        }
        img
    }
}
pub fn gaussian_blur(src: &GrayF32, radius: f32) -> GrayF32 {
    if radius <= 0.0 {
        return src.clone();
    }
    let sigma = (radius / 2.0).max(0.35);
    let ksize = ((sigma * 3.0).ceil() as usize).max(1);
    let mut kernel: Vec<f32> = (0..=ksize)
        .map(|i| {
            let d = i as f32;
            (-d * d / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum = kernel[0] + 2.0 * kernel[1..].iter().sum::<f32>();
    for k in kernel.iter_mut() {
        *k /= sum;
    }
    let tmp = convolve_rows(src, &kernel);
    let t = transpose(&tmp);
    let t2 = convolve_rows(&t, &kernel);
    transpose(&t2)
}
fn convolve_rows(src: &GrayF32, kernel: &[f32]) -> GrayF32 {
    let w = src.width as usize;
    let mut out = GrayF32::new(src.width, src.height);
    out.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, cell) in row.iter_mut().enumerate() {
            let mut acc = row_src(src, y, x) * kernel[0];
            for (i, k) in kernel[1..].iter().enumerate() {
                let d = (i + 1) as isize;
                let xl = ((x as isize) - d).clamp(0, w as isize - 1) as usize;
                let xr = ((x as isize) + d).min(w as isize - 1) as usize;
                acc += (row_src(src, y, xl) + row_src(src, y, xr)) * k;
            }
            *cell = acc;
        }
    });
    out
}
#[inline]
fn row_src(src: &GrayF32, y: usize, x: usize) -> f32 {
    src.as_slice()[y * (src.width as usize) + x]
}
pub fn transpose(src: &GrayF32) -> GrayF32 {
    let (w, h) = (src.width as usize, src.height as usize);
    let mut data = vec![0f32; w * h];
    data.par_chunks_mut(h).enumerate().for_each(|(x, col)| {
        for (y, cell) in col.iter_mut().enumerate() {
            *cell = src.as_slice()[y * w + x];
        }
    });
    GrayF32::from_vec(src.height, src.width, data)
}
pub fn moving_mean(src: &GrayF32, radius: f32) -> GrayF32 {
    let r = radius.max(1.0).round() as usize;
    let w = src.width as usize;
    let mut tmp = GrayF32::new(src.width, src.height);
    tmp.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, cell) in row.iter_mut().enumerate() {
            let mut acc = 0f32;
            let mut n = 0usize;
            let xa = x.saturating_sub(r);
            let xb = (x + r).min(w - 1);
            for i in xa..=xb {
                acc += row_src(src, y, i);
                n += 1;
            }
            *cell = acc / n as f32;
        }
    });
    let t = transpose(&tmp);
    let tw = t.width as usize;
    let mut out_t = GrayF32::new(t.width, t.height);
    out_t.data.par_chunks_mut(tw).enumerate().for_each(|(y, row)| {
        for (x, cell) in row.iter_mut().enumerate() {
            let mut acc = 0f32;
            let mut n = 0usize;
            let xa = x.saturating_sub(r);
            let xb = (x + r).min(tw - 1);
            for i in xa..=xb {
                acc += row_src(&t, y, i);
                n += 1;
            }
            *cell = acc / n as f32;
        }
    });
    transpose(&out_t)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blur_preserves_constant_image() {
        let mut g = GrayF32::new(64, 64);
        for v in g.data_mut() {
            *v = 0.5;
        }
        let out = gaussian_blur(&g, 4.0);
        assert!(out.as_slice().iter().all(|v| (v - 0.5).abs() < 1e-4));
    }
    #[test]
    fn wrap_sampling_loops() {
        let mut g = GrayF32::new(4, 4);
        g.set(0, 0, 1.0);
        assert_eq!(g.wrapped(4, 0), 1.0);
        assert_eq!(g.wrapped(-4, 0), 1.0);
        assert_eq!(g.wrapped(0, -4), 1.0);
    }
}
