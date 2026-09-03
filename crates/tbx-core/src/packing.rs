use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use crate::buffer::GrayF32;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelSource {
    Empty,
    Constant(u8),
    MapOutput(MapOutput),
    Custom,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapOutput {
    Roughness,
    Ao,
    Height,
    Normal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnginePreset {
    Custom,
    UnrealOrm,
    UnityMetallic,
    Godot,
}
pub enum ResolvedSource<'a> {
    Constant(u8),
    Gray(&'a GrayF32),
    OwnedGray(GrayF32),
}
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("channel {0} has no source assigned")]
    ChannelEmpty(char),
    #[error("source sizes do not match: {0}")]
    SizeMismatch(String),
    #[error("all channels are constant values — assign at least one image source")]
    AllConstant,
}
const CHANNEL_LABELS: [char; 4] = ['R', 'G', 'B', 'A'];
pub fn pack(width: u32, height: u32, sources: &[ResolvedSource<'_>; 4]) -> RgbaImage {
    let mut out = RgbaImage::new(width, height);
    let rows: Vec<Vec<Rgba<u8>>> = (0..height)
        .into_par_iter()
        .map(|y| {
            (0..width)
                .map(|x| {
                    let mut px = [0u8; 4];
                    for (c, s) in sources.iter().enumerate() {
                        px[c] = match s {
                            ResolvedSource::Constant(v) => *v,
                            ResolvedSource::Gray(g) => {
                                let sx = x.min(g.width.saturating_sub(1));
                                let sy = y.min(g.height.saturating_sub(1));
                                (g.at(sx, sy).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                            }
                            ResolvedSource::OwnedGray(g) => {
                                let sx = x.min(g.width.saturating_sub(1));
                                let sy = y.min(g.height.saturating_sub(1));
                                (g.at(sx, sy).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                            }
                        };
                    }
                    Rgba(px)
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
pub fn check(
    sources: &[ChannelSource; 4],
    dims: &[Option<(u32, u32)>; 4],
) -> Result<(u32, u32), PackError> {
    let mut size: Option<(u32, u32)> = None;
    for (i, src) in sources.iter().enumerate() {
        if matches!(src, ChannelSource::Empty) {
            return Err(PackError::ChannelEmpty(CHANNEL_LABELS[i]));
        }
        if let Some(d) = dims[i] {
            if let Some(s) = size {
                if s != d {
                    return Err(PackError::SizeMismatch(format!(
                        "channel {} is {}×{} but others are {}×{}",
                        CHANNEL_LABELS[i], d.0, d.1, s.0, s.1
                    )));
                }
            } else {
                size = Some(d);
            }
        }
    }
    if size.is_none() {
        return Err(PackError::AllConstant);
    }
    Ok(size.unwrap())
}
pub fn preset_mapping(preset: EnginePreset) -> [ChannelSource; 4] {
    match preset {
        EnginePreset::Custom => [ChannelSource::Empty; 4],
        EnginePreset::UnrealOrm => [
            ChannelSource::MapOutput(MapOutput::Ao),
            ChannelSource::MapOutput(MapOutput::Roughness),
            ChannelSource::Empty,
            ChannelSource::Constant(255),
        ],
        EnginePreset::UnityMetallic => [
            ChannelSource::Empty,
            ChannelSource::MapOutput(MapOutput::Ao),
            ChannelSource::Empty,
            ChannelSource::MapOutput(MapOutput::Roughness),
        ],
        EnginePreset::Godot => [
            ChannelSource::MapOutput(MapOutput::Ao),
            ChannelSource::MapOutput(MapOutput::Roughness),
            ChannelSource::Empty,
            ChannelSource::Constant(255),
        ],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn gray(w: u32, h: u32, v: f32) -> GrayF32 {
        let mut g = GrayF32::new(w, h);
        for x in g.data_mut() {
            *x = v;
        }
        g
    }
    #[test]
    fn pack_writes_channels_correctly() {
        let r = gray(8, 8, 1.0);
        let g = gray(8, 8, 0.5);
        let sources = [
            ResolvedSource::Gray(&r),
            ResolvedSource::Gray(&g),
            ResolvedSource::Constant(0),
            ResolvedSource::Constant(255),
        ];
        let img = pack(8, 8, &sources);
        let px = img.get_pixel(4, 4);
        assert_eq!(px[0], 255);
        assert!((px[1] as i32 - 128).abs() <= 1);
        assert_eq!(px[2], 0);
        assert_eq!(px[3], 255);
    }
    #[test]
    fn check_rejects_empty_channel() {
        let sources = [
            ChannelSource::MapOutput(MapOutput::Ao),
            ChannelSource::Empty,
            ChannelSource::Constant(0),
            ChannelSource::Constant(255),
        ];
        let dims = [Some((4, 4)); 4];
        assert!(matches!(check(&sources, &dims), Err(PackError::ChannelEmpty('G'))));
    }
    #[test]
    fn check_rejects_all_constant() {
        let sources = [
            ChannelSource::Constant(0),
            ChannelSource::Constant(0),
            ChannelSource::Constant(0),
            ChannelSource::Constant(255),
        ];
        let dims = [None; 4];
        assert!(matches!(check(&sources, &dims), Err(PackError::AllConstant)));
    }
    #[test]
    fn check_rejects_mismatched_sizes() {
        let sources = [
            ChannelSource::MapOutput(MapOutput::Ao),
            ChannelSource::MapOutput(MapOutput::Roughness),
            ChannelSource::Constant(0),
            ChannelSource::Constant(255),
        ];
        let dims = [Some((4, 4)), Some((8, 8)), None, None];
        assert!(matches!(check(&sources, &dims), Err(PackError::SizeMismatch(_))));
    }
    #[test]
    fn unreal_preset_layout() {
        let m = preset_mapping(EnginePreset::UnrealOrm);
        assert_eq!(m[0], ChannelSource::MapOutput(MapOutput::Ao));
        assert_eq!(m[1], ChannelSource::MapOutput(MapOutput::Roughness));
        assert_eq!(m[3], ChannelSource::Constant(255));
    }
}
