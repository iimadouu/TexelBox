pub mod ao;
pub mod height;
pub mod normal;
pub mod params;
pub mod roughness;
pub use params::{AoParams, HeightParams, HeightSource, MapSetParams, NormalKernel, NormalParams, RoughnessParams};
use image::RgbaImage;
use crate::buffer::{GrayF32};
#[derive(Clone, Debug, Default)]
pub struct MapOutputs {
    pub height: Option<GrayF32>,
    pub normal: Option<RgbaImage>,
    pub roughness: Option<GrayF32>,
    pub ao: Option<GrayF32>,
}
impl MapOutputs {
    pub fn generated_count(&self) -> usize {
        [self.height.is_some(), self.normal.is_some(), self.roughness.is_some(), self.ao.is_some()]
            .iter()
            .filter(|v| **v)
            .count()
    }
}
fn extract_source(source: &RgbaImage, p: &HeightParams) -> GrayF32 {
    match p.source {
        HeightSource::Luminance => GrayF32::from_rgba(source),
        HeightSource::Red   => GrayF32::from_rgba_channel(source, 0),
        HeightSource::Green => GrayF32::from_rgba_channel(source, 1),
        HeightSource::Blue  => GrayF32::from_rgba_channel(source, 2),
        HeightSource::Alpha => GrayF32::from_rgba_channel(source, 3),
    }
}
pub fn generate_all(source: &RgbaImage, params: &MapSetParams) -> MapOutputs {
    let raw = extract_source(source, &params.height);
    let mut out = MapOutputs::default();
    let height = height::generate(&raw, &params.height);
    if params.enable_normal {
        out.normal = Some(normal::generate(&height, &params.normal));
    }
    if params.enable_roughness {
        out.roughness = Some(roughness::generate(&height, &params.roughness));
    }
    if params.enable_ao {
        out.ao = Some(ao::generate(&height, &params.ao));
    }
    if params.enable_height {
        out.height = Some(height);
    }
    out
}
