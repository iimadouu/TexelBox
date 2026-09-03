use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeightSource {
    Luminance,
    Red,
    Green,
    Blue,
    Alpha,
}
impl Default for HeightSource {
    fn default() -> Self {
        Self::Luminance
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HeightParams {
    pub source: HeightSource,
    pub contrast: f32,
    pub brightness: f32,
    pub blur_radius: f32,
    pub invert: bool,
}
impl Default for HeightParams {
    fn default() -> Self {
        Self {
            source: HeightSource::default(),
            contrast: 1.0,
            brightness: 0.0,
            blur_radius: 2.0,
            invert: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormalKernel {
    K3,
    K5,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NormalParams {
    pub strength: f32,
    pub kernel: NormalKernel,
    pub wrap_edges: bool,
    pub auto_strength: bool,
    pub detail_enhance: bool,
}
impl Default for NormalParams {
    fn default() -> Self {
        Self {
            strength: 1.5,
            kernel: NormalKernel::K3,
            wrap_edges: false,
            auto_strength: true,
            detail_enhance: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RoughnessParams {
    pub radius: f32,
    pub amount: f32,
    pub curve: f32,
    pub invert: bool,
}
impl Default for RoughnessParams {
    fn default() -> Self {
        Self { radius: 4.0, amount: 1.5, curve: 1.0, invert: false }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AoParams {
    pub radius: f32,
    pub intensity: f32,
}
impl Default for AoParams {
    fn default() -> Self {
        Self { radius: 10.0, intensity: 0.9 }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapSetParams {
    pub height: HeightParams,
    pub normal: NormalParams,
    pub roughness: RoughnessParams,
    pub ao: AoParams,
    pub enable_height: bool,
    pub enable_normal: bool,
    pub enable_roughness: bool,
    pub enable_ao: bool,
}
impl Default for MapSetParams {
    fn default() -> Self {
        Self {
            height: HeightParams::default(),
            normal: NormalParams::default(),
            roughness: RoughnessParams::default(),
            ao: AoParams::default(),
            enable_height: true,
            enable_normal: true,
            enable_roughness: true,
            enable_ao: true,
        }
    }
}
