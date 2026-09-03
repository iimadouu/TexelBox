use image::{imageops::FilterType, RgbaImage};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resampling {
    Bilinear,
    Bicubic,
    Lanczos3,
}
impl Resampling {
    pub fn filter(self) -> FilterType {
        match self {
            Self::Bilinear => FilterType::Triangle,
            Self::Bicubic => FilterType::CatmullRom,
            Self::Lanczos3 => FilterType::Lanczos3,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PotSnap {
    None,
    Nearest,
    Up,
    Down,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DdsCompression {
    Bc1,
    Bc3,
    Bc5,
    Bc7,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OptimizeParams {
    pub target_size: Option<u32>,
    pub pot_snap: PotSnap,
    pub resampling: Resampling,
    pub dds: Option<DdsCompression>,
}
impl Default for OptimizeParams {
    fn default() -> Self {
        Self { target_size: None, pot_snap: PotSnap::None, resampling: Resampling::Lanczos3, dds: None }
    }
}
pub fn snap_pot(v: u32, mode: PotSnap) -> u32 {
    if v == 0 {
        return 1;
    }
    match mode {
        PotSnap::None => v,
        PotSnap::Up => v.next_power_of_two(),
        PotSnap::Down => {
            if v.is_power_of_two() {
                v
            } else {
                v.next_power_of_two() / 2
            }
        }
        PotSnap::Nearest => {
            if v.is_power_of_two() {
                v
            } else {
                let up = v.next_power_of_two();
                let down = up / 2;
                if up - v <= v - down {
                    up
                } else {
                    down
                }
            }
        }
    }
}
pub fn target_dimensions(w: u32, h: u32, params: &OptimizeParams) -> (u32, u32) {
    let (bw, bh) = match params.target_size {
        Some(s) => (s, s),
        None => (w, h),
    };
    (snap_pot(bw, params.pot_snap), snap_pot(bh, params.pot_snap))
}
pub fn resize(img: &RgbaImage, params: &OptimizeParams) -> RgbaImage {
    let (w, h) = img.dimensions();
    let (tw, th) = target_dimensions(w, h, params);
    if tw == w && th == h {
        return img.clone();
    }
    image::imageops::resize(img, tw.max(1), th.max(1), params.resampling.filter())
}
pub fn extension(params: &OptimizeParams) -> &'static str {
    match params.dds {
        Some(_) => "dds",
        None => "png",
    }
}
pub fn sanitize_filename(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if trimmed.is_empty() || trimmed.chars().all(|c| c == '_') {
        "untitled".into()
    } else {
        trimmed
    }
}
pub fn render_name(template: &str, name: &str, map: &str, ext: &str) -> String {
    template.replace("{name}", name).replace("{map}", map).replace("{ext}", ext)
}
pub fn encode_dds(img: &RgbaImage, compression: DdsCompression, fast: bool) -> Result<Vec<u8>, crate::io::CoreError> {
    use rusty_dds::{Dds, DecodeContent, EncodeLayout, EncodeQuality};
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err(crate::io::CoreError::Other("cannot encode an empty image".into()));
    }
    let content = match compression {
        DdsCompression::Bc1 => DecodeContent::Bc1,
        DdsCompression::Bc3 => DecodeContent::Bc3,
        DdsCompression::Bc5 => DecodeContent::Bc5UNorm,
        DdsCompression::Bc7 => DecodeContent::Bc7,
    };
    let max_side = w.max(h);
    let mips = 32 - max_side.leading_zeros();
    let quality = if fast { EncodeQuality::Fast } else { EncodeQuality::Quality };
    let layout = EncodeLayout::flat_2d(content, w, h).with_mips(mips).with_quality(quality);
    let dds = Dds::encode_from_rgba8(img.as_raw(), layout)
        .map_err(|e| crate::io::CoreError::Write(format!("DDS encode failed: {e}")))?;
    let mut buf = Vec::new();
    dds.write(&mut buf)
        .map_err(|e| crate::io::CoreError::Write(format!("DDS serialize failed: {e}")))?;
    Ok(buf)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rgb(w: u32, h: u32) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([(x * 4) as u8, (y * 4) as u8, 128, 255]);
        }
        img
    }
    #[test]
    fn pot_snap_modes() {
        assert_eq!(snap_pot(100, PotSnap::None), 100);
        assert_eq!(snap_pot(100, PotSnap::Up), 128);
        assert_eq!(snap_pot(100, PotSnap::Down), 64);
        assert_eq!(snap_pot(100, PotSnap::Nearest), 128);
        assert_eq!(snap_pot(200, PotSnap::Nearest), 256);
        assert_eq!(snap_pot(193, PotSnap::Nearest), 256);
        assert_eq!(snap_pot(150, PotSnap::Nearest), 128);
        assert_eq!(snap_pot(256, PotSnap::Up), 256);
        assert_eq!(snap_pot(256, PotSnap::Down), 256);
        assert_eq!(snap_pot(256, PotSnap::Nearest), 256);
        assert_eq!(snap_pot(0, PotSnap::Up), 1);
    }
    #[test]
    fn target_dimensions_applies_size_then_snap() {
        let p = OptimizeParams { target_size: Some(300), pot_snap: PotSnap::Up, ..Default::default() };
        assert_eq!(target_dimensions(1024, 512, &p), (512, 512));
        let p = OptimizeParams { target_size: None, pot_snap: PotSnap::Down, ..Default::default() };
        assert_eq!(target_dimensions(1000, 600, &p), (512, 512));
    }
    #[test]
    fn resize_to_target() {
        let img = rgb(256, 256);
        let p = OptimizeParams { target_size: Some(64), ..Default::default() };
        let out = resize(&img, &p);
        assert_eq!(out.dimensions(), (64, 64));
    }
    #[test]
    fn resize_noop_clones() {
        let img = rgb(64, 64);
        let out = resize(&img, &OptimizeParams::default());
        assert_eq!(out.dimensions(), (64, 64));
        assert_eq!(out.as_raw(), img.as_raw());
    }
    #[test]
    fn render_name_tokens() {
        assert_eq!(render_name("{name}_{map}.{ext}", "hero", "normal", "png"), "hero_normal.png");
        assert_eq!(render_name("{name}_{map}.{ext}", "hero", "albedo", "dds"), "hero_albedo.dds");
        assert_eq!(render_name("{name}.{ext}", "hero", "", "tga"), "hero.tga");
        assert_eq!(render_name("{name}-{unknown}", "hero", "", "png"), "hero-{unknown}");
    }
    #[test]
    fn sanitize_rejects_path_escapes() {
        assert_eq!(sanitize_filename("a/b\\c:d"), "a_b_c_d");
        assert_eq!(sanitize_filename("name.. "), "name");
        assert_eq!(sanitize_filename("???"), "untitled");
        assert_eq!(sanitize_filename("ok_name"), "ok_name");
    }
    #[test]
    fn encode_dds_produces_valid_container() {
        use rusty_dds::DdsView;
        let img = rgb(64, 64);
        let bytes = encode_dds(&img, DdsCompression::Bc3, true).expect("BC3 encode");
        assert_eq!(&bytes[0..4], b"DDS ");
        let view = DdsView::parse(&bytes).expect("parse back");
        assert_eq!(view.header.height, 64);
        assert_eq!(view.header.width, 64);
    }
}
