use crate::plan::Plan;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    AtlasUnlimitedImages,
    AtlasSize8192,
    AtlasBleedPaddingControl,
    AtlasRotationPacking,
    AtlasSidecarFormatsExtra,
    AtlasImageFormats,
    AtlasTrimSheetMode,
    AtlasPriorityArrange,
    MapsRoughnessMap,
    MapsAoMap,
    MapsFullSliderControl,
    MapsHighResolution,
    MapsBatchGeneration,
    TileableAutoHeal,
    TileableLiveRepeatPreview,
    TileableUnlimitedResolution,
    ChannelPackEnginePresets,
    ChannelPackBatch,
    ChannelPackAlphaChannel,
    OptimizeDdsCompression,
    OptimizeBatchTemplateExport,
    OptimizeResamplingChoice,
    PreviewSphereViewport,
    PreviewMultipleLightingRigs,
    PreviewFullValidationSuite,
    BatchChaining,
    BatchUnlimitedFiles,
    BatchDryRun,
    BatchScheduledProcessing,
    PresetsUnlimited,
    PresetsExportImport,
    EngineExportProfiles,
}
impl Capability {
    pub fn required_plan(self) -> Plan {
        match self {
            Self::MapsRoughnessMap
            | Self::MapsAoMap
            | Self::MapsFullSliderControl => Plan::Free,
            Self::TileableAutoHeal => Plan::Free,
            Self::ChannelPackAlphaChannel => Plan::Free,
            Self::AtlasUnlimitedImages
            | Self::AtlasSize8192
            | Self::AtlasBleedPaddingControl
            | Self::AtlasRotationPacking
            | Self::AtlasSidecarFormatsExtra
            | Self::AtlasImageFormats
            | Self::AtlasTrimSheetMode
            | Self::AtlasPriorityArrange
            | Self::MapsHighResolution
            | Self::MapsBatchGeneration
            | Self::TileableLiveRepeatPreview
            | Self::TileableUnlimitedResolution
            | Self::ChannelPackEnginePresets
            | Self::ChannelPackBatch
            | Self::OptimizeDdsCompression
            | Self::OptimizeBatchTemplateExport
            | Self::OptimizeResamplingChoice
            | Self::PreviewSphereViewport
            | Self::PreviewMultipleLightingRigs
            | Self::PreviewFullValidationSuite
            | Self::BatchChaining
            | Self::BatchUnlimitedFiles
            | Self::BatchDryRun
            | Self::BatchScheduledProcessing
            | Self::PresetsUnlimited
            | Self::PresetsExportImport
            | Self::EngineExportProfiles => Plan::Pro,
        }
    }
    pub fn all() -> &'static [Capability] {
        const ALL: &[Capability] = &[
            Capability::AtlasUnlimitedImages,
            Capability::AtlasSize8192,
            Capability::AtlasBleedPaddingControl,
            Capability::AtlasRotationPacking,
            Capability::AtlasSidecarFormatsExtra,
            Capability::AtlasImageFormats,
            Capability::AtlasTrimSheetMode,
            Capability::AtlasPriorityArrange,
            Capability::MapsRoughnessMap,
            Capability::MapsAoMap,
            Capability::MapsFullSliderControl,
            Capability::MapsHighResolution,
            Capability::MapsBatchGeneration,
            Capability::TileableAutoHeal,
            Capability::TileableLiveRepeatPreview,
            Capability::TileableUnlimitedResolution,
            Capability::ChannelPackEnginePresets,
            Capability::ChannelPackBatch,
            Capability::ChannelPackAlphaChannel,
            Capability::OptimizeDdsCompression,
            Capability::OptimizeBatchTemplateExport,
            Capability::OptimizeResamplingChoice,
            Capability::PreviewSphereViewport,
            Capability::PreviewMultipleLightingRigs,
            Capability::PreviewFullValidationSuite,
            Capability::BatchChaining,
            Capability::BatchUnlimitedFiles,
            Capability::BatchDryRun,
            Capability::BatchScheduledProcessing,
            Capability::PresetsUnlimited,
            Capability::PresetsExportImport,
            Capability::EngineExportProfiles,
        ];
        ALL
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::AtlasUnlimitedImages => "atlas.unlimited_images",
            Self::AtlasSize8192 => "atlas.size_8192",
            Self::AtlasBleedPaddingControl => "atlas.bleed_padding",
            Self::AtlasRotationPacking => "atlas.rotation",
            Self::AtlasSidecarFormatsExtra => "atlas.sidecar_formats",
            Self::AtlasImageFormats => "atlas.image_formats",
            Self::AtlasTrimSheetMode => "atlas.trim_sheet",
            Self::AtlasPriorityArrange => "atlas.priority_arrange",
            Self::MapsRoughnessMap => "maps.roughness",
            Self::MapsAoMap => "maps.ao",
            Self::MapsFullSliderControl => "maps.full_sliders",
            Self::MapsHighResolution => "maps.high_res",
            Self::MapsBatchGeneration => "maps.batch",
            Self::TileableAutoHeal => "tileable.auto_heal",
            Self::TileableLiveRepeatPreview => "tileable.live_repeat",
            Self::TileableUnlimitedResolution => "tileable.unlimited_res",
            Self::ChannelPackEnginePresets => "pack.engine_presets",
            Self::ChannelPackBatch => "pack.batch",
            Self::ChannelPackAlphaChannel => "pack.alpha",
            Self::OptimizeDdsCompression => "optimize.dds",
            Self::OptimizeBatchTemplateExport => "optimize.batch_template",
            Self::OptimizeResamplingChoice => "optimize.resampling",
            Self::PreviewSphereViewport => "preview.sphere",
            Self::PreviewMultipleLightingRigs => "preview.lighting_rigs",
            Self::PreviewFullValidationSuite => "preview.full_validation",
            Self::BatchChaining => "batch.chaining",
            Self::BatchUnlimitedFiles => "batch.unlimited_files",
            Self::BatchDryRun => "batch.dry_run",
            Self::BatchScheduledProcessing => "batch.scheduled",
            Self::PresetsUnlimited => "presets.unlimited",
            Self::PresetsExportImport => "presets.export_import",
            Self::EngineExportProfiles => "engine.export_profiles",
        }
    }
    pub fn from_key(key: &str) -> Option<Capability> {
        Self::all().iter().copied().find(|c| c.key() == key)
    }
}
