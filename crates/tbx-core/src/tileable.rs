use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TileableParams {
    pub auto_heal_strength: f32,
    pub heal_radius: f32,
    pub heal_passes: u8,
    pub mirror_mode: bool,
    pub brush_size: f32,
}
impl Default for TileableParams {
    fn default() -> Self {
        Self { auto_heal_strength: 0.85, heal_radius: 32.0, heal_passes: 2, mirror_mode: false, brush_size: 24.0 }
    }
}
#[inline]
fn sample_wrapped(img: &RgbaImage, x: i64, y: i64) -> Rgba<u8> {
    let w = img.width() as i64;
    let h = img.height() as i64;
    *img.get_pixel(x.rem_euclid(w) as u32, y.rem_euclid(h) as u32)
}
pub fn offset_wrap(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let ox = (w / 2) as i64;
    let oy = (h / 2) as i64;
    let mut out = RgbaImage::new(w, h);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            out.put_pixel(x as u32, y as u32, sample_wrapped(img, x + ox, y + oy));
        }
    }
    out
}
pub fn mirror_tile(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let ew = w & !1;
    let eh = h & !1;
    let hw = ew / 2;
    let hh = eh / 2;
    let mut out = RgbaImage::new(ew, eh);
    for y in 0..eh {
        for x in 0..ew {
            let sx = if x < hw { x } else { ew - 1 - x };
            let sy = if y < hh { y } else { eh - 1 - y };
            out.put_pixel(x, y, *img.get_pixel(sx, sy));
        }
    }
    out
}
pub fn auto_heal(img: &RgbaImage, strength: f32, radius: f32, passes: u8) -> RgbaImage {
    let mut current = img.clone();
    for _ in 0..passes.max(1) {
        current = heal_once(&current, strength, radius);
    }
    current
}
fn heal_once(img: &RgbaImage, strength: f32, radius: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let r = radius.max(1.0);
    let s = strength.clamp(0.0, 1.0);
    let mirrored = mirror_tile(img);
    let mut out = RgbaImage::new(w, h);
    let rows: Vec<Vec<Rgba<u8>>> = (0..h as i64)
        .into_par_iter()
        .map(|y| {
            (0..w as i64)
                .map(|x| {
                    let dx = ((x - (w as i64) / 2).abs() as f32).min(r);
                    let dy = ((y - (h as i64) / 2).abs() as f32).min(r);
                    let wx = 0.5 + 0.5 * (dx / r * std::f32::consts::PI).cos();
                    let wy = 0.5 + 0.5 * (dy / r * std::f32::consts::PI).cos();
                    let weight = s * wx.max(wy).powf(1.5);
                    let a = *img.get_pixel(x as u32, y as u32);
                    let b = *mirrored.get_pixel(x as u32, y as u32);
                    blend(a, b, weight)
                })
                .collect()
        })
        .collect();
    for (y, row) in rows.iter().enumerate() {
        for (x, px) in row.iter().enumerate() {
            out.put_pixel(x as u32, y as u32, *px);
        }
    }
    out
}
#[inline]
fn blend(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    let mix = |ca: u8, cb: u8| {
        let v = ca as f32 * (1.0 - t) + cb as f32 * t;
        v.round().clamp(0.0, 255.0) as u8
    };
    Rgba([mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2]), mix(a[3], b[3])])
}
pub fn brush_clone(
    img: &mut RgbaImage,
    dest: (i64, i64),
    source_offset: (i64, i64),
    radius: f32,
    hardness: f32,
) {
    let r = radius.max(1.0);
    let hard = hardness.clamp(0.0, 1.0);
    let (dx, dy) = dest;
    let (ox, oy) = source_offset;
    for py in (dy - r as i64)..=(dy + r as i64) {
        for px in (dx - r as i64)..=(dx + r as i64) {
            let fx = (px - dx) as f32 / r;
            let fy = (py - dy) as f32 / r;
            let d = (fx * fx + fy * fy).sqrt();
            if d > 1.0 {
                continue;
            }
            let t = if d <= hard { 1.0 } else { 1.0 - (d - hard) / (1.0 - hard).max(0.001) };
            if t <= 0.0 {
                continue;
            }
            let src = sample_wrapped(img, px + ox, py + oy);
            let dst = *img.get_pixel(px.rem_euclid(img.width() as i64) as u32, py.rem_euclid(img.height() as i64) as u32);
            let mixed = blend(dst, src, t);
            img.put_pixel(px.rem_euclid(img.width() as i64) as u32, py.rem_euclid(img.height() as i64) as u32, mixed);
        }
    }
}
pub fn tile_preview(img: &RgbaImage, n: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let n = n.max(1);
    let mut out = RgbaImage::new(w * n, h * n);
    for ty in 0..n {
        for tx in 0..n {
            for (x, y, px) in img.enumerate_pixels() {
                out.put_pixel(tx * w + x, ty * h + y, *px);
            }
        }
    }
    out
}
pub fn seam_error(img: &RgbaImage) -> f32 {
    let (w, h) = img.dimensions();
    let mut acc = 0f64;
    let mut n = 0usize;
    for x in 0..w {
        acc += diff(img.get_pixel(x, 0), img.get_pixel(x, h - 1));
        n += 1;
    }
    for y in 0..h {
        acc += diff(img.get_pixel(0, y), img.get_pixel(w - 1, y));
        n += 1;
    }
    (acc / n.max(1) as f64) as f32 / 255.0
}
fn diff(a: &Rgba<u8>, b: &Rgba<u8>) -> f64 {
    let dr = a[0] as f64 - b[0] as f64;
    let dg = a[1] as f64 - b[1] as f64;
    let db = a[2] as f64 - b[2] as f64;
    ((dr * dr + dg * dg + db * db) / 3.0).sqrt()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn gradient_image(w: u32, h: u32) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, 128, 255]));
            }
        }
        img
    }
    #[test]
    fn offset_wrap_is_reversible() {
        let img = gradient_image(16, 16);
        let twice = offset_wrap(&offset_wrap(&img));
        assert_eq!(&twice, &img);
    }
    #[test]
    fn mirror_mode_has_zero_seam_error() {
        let img = gradient_image(32, 32);
        let mirrored = mirror_tile(&img);
        assert!(seam_error(&mirrored) < 0.02);
    }
    #[test]
    fn mirror_tile_odd_dimensions_are_snapped_to_even() {
        let img = gradient_image(33, 33);
        let mirrored = mirror_tile(&img);
        assert_eq!(mirrored.dimensions(), (32, 32), "odd dims snapped to even");
        assert!(seam_error(&mirrored) < 0.02, "snapped mirror must have zero seam error");
    }
    #[test]
    fn auto_heal_reduces_seam_error() {
        let img = gradient_image(32, 32);
        let wrapped = offset_wrap(&img);
        let before = seam_error(&wrapped);
        let healed = auto_heal(&wrapped, 0.9, 8.0, 2);
        let after = seam_error(&healed);
        assert!(after < before, "heal should reduce seam error ({before} -> {after})");
    }
    #[test]
    fn tile_preview_dimensions() {
        let img = gradient_image(8, 8);
        let tiled = tile_preview(&img, 3);
        assert_eq!(tiled.dimensions(), (24, 24));
    }
}
