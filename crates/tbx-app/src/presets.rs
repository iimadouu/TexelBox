use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tbx_core::batch::BatchStep;
use tbx_core::maps::MapSetParams;
use tbx_core::packing::ChannelSource;
pub const STORE_VERSION: u32 = 1;
pub const FREE_MAX_PER_FEATURE: usize = 5;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    Maps,
    Tileable,
    Packing,
    Atlas,
    Optimize,
    Batch,
    Project,
}
impl Feature {
    pub fn from_key(key: &str) -> Option<Feature> {
        match key {
            "maps" => Some(Feature::Maps),
            "tileable" => Some(Feature::Tileable),
            "packing" => Some(Feature::Packing),
            "atlas" => Some(Feature::Atlas),
            "optimize" => Some(Feature::Optimize),
            "batch" => Some(Feature::Batch),
            "project" => Some(Feature::Project),
            _ => None,
        }
    }
    pub fn name_key(self) -> &'static str {
        match self {
            Feature::Maps => "presets-feature-maps",
            Feature::Tileable => "presets-feature-tileable",
            Feature::Packing => "presets-feature-packing",
            Feature::Atlas => "presets-feature-atlas",
            Feature::Optimize => "presets-feature-optimize",
            Feature::Batch => "presets-feature-batch",
            Feature::Project => "presets-feature-project",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TileablePreset {
    pub heal_strength: f32,
    pub heal_radius: f32,
    pub heal_passes: u8,
    pub mirror_mode: bool,
    pub brush_size: f32,
    pub brush_offset_x: f32,
    pub brush_offset_y: f32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PackingPreset {
    pub channels: [ChannelSource; 4],
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AtlasPreset {
    pub size_index: i32,
    pub padding: u32,
    pub bleed: u32,
    pub rotation: bool,
    pub trim: bool,
    pub sidecar: i32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OptimizePreset {
    pub size_index: i32,
    pub snap_index: i32,
    pub resampling_index: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PresetPayload {
    Maps(MapSetParams),
    Tileable(TileablePreset),
    Packing(PackingPreset),
    Atlas(AtlasPreset),
    Optimize(OptimizePreset),
    Batch { steps: Vec<BatchStep> },
    Project { entries: Vec<(String, PresetPayload)> },
}
impl PresetPayload {
    pub fn feature(&self) -> Feature {
        match self {
            PresetPayload::Maps(_) => Feature::Maps,
            PresetPayload::Tileable(_) => Feature::Tileable,
            PresetPayload::Packing(_) => Feature::Packing,
            PresetPayload::Atlas(_) => Feature::Atlas,
            PresetPayload::Optimize(_) => Feature::Optimize,
            PresetPayload::Batch { .. } => Feature::Batch,
            PresetPayload::Project { .. } => Feature::Project,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub payload: PresetPayload,
    pub last_used: i64,
}
#[derive(Serialize, Deserialize)]
struct StoreFile {
    version: u32,
    presets: Vec<Preset>,
}
pub fn store_path() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("app", "TexelBox", "TexelBox")?;
    Some(dirs.config_dir().join("presets.json"))
}
pub fn load_all() -> Vec<Preset> {
    let Some(path) = store_path() else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    match serde_json::from_str::<StoreFile>(&text) {
        Ok(file) if file.version == STORE_VERSION => file.presets,
        _ => Vec::new(),
    }
}
pub fn save_all(presets: &[Preset]) -> Result<(), String> {
    let path = store_path().ok_or_else(|| "no OS config directory".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = StoreFile { version: STORE_VERSION, presets: presets.to_vec() };
    let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}
pub fn export_text(preset: &Preset) -> Result<String, String> {
    serde_json::to_string_pretty(preset).map_err(|e| e.to_string())
}
pub fn import_text(text: &str) -> Result<Preset, String> {
    let mut preset: Preset = serde_json::from_str(text).map_err(|e| e.to_string())?;
    preset.name = sanitize_name(&preset.name);
    if preset.name.is_empty() {
        return Err("preset has no name".to_string());
    }
    if let PresetPayload::Project { entries } = &preset.payload {
        for (key, payload) in entries {
            if Feature::from_key(key).is_none() {
                return Err(format!("unknown functionality key: {key}"));
            }
            if matches!(payload, PresetPayload::Project { .. }) {
                return Err("nested project presets are not allowed".to_string());
            }
        }
    }
    Ok(preset)
}
pub fn sanitize_name(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    use tbx_core::batch::{BatchFormat, BatchStep, OptimizeStepParams, TileableStepMode};
    use tbx_core::optimize::{DdsCompression, OptimizeParams};
    use tbx_core::packing::{EnginePreset, MapOutput};
    #[test]
    fn full_chain_roundtrips_through_json() {
        let preset = Preset {
            name: "test chain".to_string(),
            payload: PresetPayload::Batch {
                steps: vec![
                    BatchStep::Tileable(TileableStepMode::Offset),
                    BatchStep::Maps(MapSetParams::default()),
                    BatchStep::Pack(EnginePreset::UnrealOrm),
                    BatchStep::Optimize(OptimizeStepParams {
                        resize: OptimizeParams::default(),
                        format: BatchFormat::Dds,
                        bc: DdsCompression::Bc7,
                    }),
                ],
            },
            last_used: 42,
        };
        let text = export_text(&preset).expect("export failed");
        let back = import_text(&text).unwrap();
        assert_eq!(back.name, "test chain");
        assert!(matches!(back.payload, PresetPayload::Batch { ref steps } if steps.len() == 4));
    }
    #[test]
    fn project_bundle_roundtrips_and_rejects_nesting() {
        let bundle = PresetPayload::Project {
            entries: vec![
                ("maps".into(), PresetPayload::Maps(MapSetParams::default())),
                (
                    "packing".into(),
                    PresetPayload::Packing(PackingPreset {
                        channels: [
                            ChannelSource::MapOutput(MapOutput::Ao),
                            ChannelSource::MapOutput(MapOutput::Roughness),
                            ChannelSource::Constant(0),
                            ChannelSource::Constant(255),
                        ],
                    }),
                ),
            ],
        };
        let preset = Preset { name: "proj".into(), payload: bundle, last_used: 1 };
        let text = export_text(&preset).expect("export failed");
        let back = import_text(&text).unwrap();
        assert!(matches!(back.payload, PresetPayload::Project { ref entries } if entries.len() == 2));
        let nested = Preset {
            name: "bad".into(),
            payload: PresetPayload::Project {
                entries: vec![("project".into(), PresetPayload::Project { entries: Vec::new() })],
            },
            last_used: 1,
        };
        assert!(import_text(&export_text(&nested).unwrap()).is_err());
    }
    #[test]
    fn unknown_kind_is_rejected() {
        assert!(import_text(r#"{"name":"x","last_used":0,"payload":{"kind":"nope"}}"#).is_err());
    }
}
