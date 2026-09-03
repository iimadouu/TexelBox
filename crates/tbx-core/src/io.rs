use std::path::Path;
use image::{DynamicImage, RgbaImage};
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("failed to read image: {0}")]
    Read(String),
    #[error("unsupported or corrupt image: {0}")]
    Decode(String),
    #[error("failed to write image: {0}")]
    Write(String),
    #[error("unsupported format: {0}")]
    Format(String),
    #[error("{0}")]
    Other(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Tga,
    Dds,
}
impl ImageFormat {
    pub fn from_path(path: &Path) -> Option<ImageFormat> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "png" => Some(ImageFormat::Png),
            "tga" | "targa" => Some(ImageFormat::Tga),
            "dds" => Some(ImageFormat::Dds),
            _ => None,
        }
    }
}
pub fn load(path: &Path) -> Result<RgbaImage, CoreError> {
    let img: DynamicImage = image::open(path)
        .map_err(|e| CoreError::Decode(format!("{path:?}: {e}")))?;
    Ok(img.to_rgba8())
}
pub fn save(path: &Path, img: &RgbaImage) -> Result<(), CoreError> {
    let format = ImageFormat::from_path(path)
        .ok_or_else(|| CoreError::Format(format!("unsupported extension for {:?}", path)))?;
    save_with(path, img, format, None)
}
pub fn save_with(
    path: &Path,
    img: &RgbaImage,
    format: ImageFormat,
    dds_compression: Option<crate::optimize::DdsCompression>,
) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::Write(e.to_string()))?;
        }
    }
    let dyn_img = DynamicImage::ImageRgba8(img.clone());
    match format {
        ImageFormat::Png => {
            dyn_img
                .save_with_format(path, image::ImageFormat::Png)
                .map_err(|e| CoreError::Write(format!("{path:?}: {e}")))
        }
        ImageFormat::Tga => {
            dyn_img
                .save_with_format(path, image::ImageFormat::Tga)
                .map_err(|e| CoreError::Write(format!("{path:?}: {e}")))
        }
        ImageFormat::Dds => {
            use crate::optimize::{encode_dds, DdsCompression};
            let codec = dds_compression.unwrap_or(DdsCompression::Bc3);
            let bytes = encode_dds(img, codec, true)
                .map_err(|e| CoreError::Write(format!("DDS encode: {e}")))?;
            std::fs::write(path, bytes).map_err(|e| CoreError::Write(e.to_string()))
        }
    }
}
pub fn downscale_to_fit(img: &RgbaImage, max_side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w <= max_side && h <= max_side {
        return img.clone();
    }
    DynamicImage::ImageRgba8(img.clone())
        .resize(max_side, max_side, image::imageops::FilterType::Lanczos3)
        .to_rgba8()
}
