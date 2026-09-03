use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use image::RgbaImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use crate::buffer::GrayF32;
use crate::io::{self, CoreError, ImageFormat};
use crate::maps::{self, MapOutputs, MapSetParams};
use crate::optimize::{self, DdsCompression, OptimizeParams};
use crate::packing::{self, ChannelSource, EnginePreset, MapOutput, ResolvedSource};
use crate::tileable;
const INPUT_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "tga", "bmp", "webp", "gif"];
const PACK_CHANNELS: [char; 4] = ['R', 'G', 'B', 'A'];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileableStepMode {
    Offset,
    Mirror,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchFormat {
    Png,
    Tga,
    Dds,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OptimizeStepParams {
    pub resize: OptimizeParams,
    pub format: BatchFormat,
    pub bc: DdsCompression,
}
impl Default for OptimizeStepParams {
    fn default() -> Self {
        Self { resize: OptimizeParams::default(), format: BatchFormat::Png, bc: DdsCompression::Bc3 }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BatchStep {
    Tileable(TileableStepMode),
    Maps(MapSetParams),
    Pack(EnginePreset),
    Optimize(OptimizeStepParams),
}
impl BatchStep {
    pub fn name_key(&self) -> &'static str {
        match self {
            BatchStep::Tileable(_) => "batch-step-tileable",
            BatchStep::Maps(_) => "batch-step-maps",
            BatchStep::Pack(_) => "batch-step-pack",
            BatchStep::Optimize(_) => "batch-step-optimize",
        }
    }
}
#[derive(Clone, Debug)]
pub enum StepFailure {
    NoMapsEnabled,
    PackWithoutMaps,
    MapMissing(&'static str),
    UnsupportedChannel(char),
    SizeMismatch(String),
}
#[derive(Clone, Debug)]
pub enum FileOutcome {
    Done { outputs: Vec<PathBuf> },
    LoadFailed(String),
    StepFailed { index: usize, step: &'static str, reason: StepFailure },
    SaveFailed(String),
}
#[derive(Clone, Debug)]
pub struct FileResult {
    pub rel_path: String,
    pub outcome: FileOutcome,
}
impl FileResult {
    pub fn ok(&self) -> bool {
        matches!(self.outcome, FileOutcome::Done { .. })
    }
}
#[derive(Clone, Debug, Default)]
pub struct BatchReport {
    pub results: Vec<FileResult>,
}
impl BatchReport {
    pub fn ok_count(&self) -> usize {
        self.results.iter().filter(|r| r.ok()).count()
    }
}
#[derive(Clone, Copy, Debug)]
pub struct BatchOptions {
    pub max_files: Option<usize>,
    pub dry_run: bool,
    pub parallel: bool,
}
pub fn collect_inputs(input_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(input_dir, &mut out);
    out.sort();
    out
}
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if INPUT_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
                out.push(path);
            }
        }
    }
}
pub fn validate_chain(steps: &[BatchStep]) -> Option<usize> {
    let mut saw_maps = false;
    for (i, s) in steps.iter().enumerate() {
        match s {
            BatchStep::Maps(_) => saw_maps = true,
            BatchStep::Pack(_) if !saw_maps => return Some(i),
            _ => {}
        }
    }
    None
}
pub fn run_batch(
    input_dir: &Path,
    output_dir: &Path,
    steps: &[BatchStep],
    opts: &BatchOptions,
    on_file: &(impl Fn(usize, usize, &FileResult) + Send + Sync),
) -> BatchReport {
    let mut files = collect_inputs(input_dir);
    if let Some(max) = opts.max_files {
        files.truncate(max);
    }
    if opts.dry_run {
        files.truncate(1);
    }
    let out_root: PathBuf = if opts.dry_run {
        output_dir.join("dry-run")
    } else {
        output_dir.to_path_buf()
    };
    let total = files.len();
    if total == 0 {
        return BatchReport::default();
    }
    let counter = AtomicUsize::new(0);
    let process = |path: &PathBuf, idx: usize| {
        let rel_dir = path
            .strip_prefix(input_dir)
            .ok()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| Path::new(""));
        let result = process_file(path, &out_root, rel_dir, steps);
        let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
        on_file(done, total, &result);
        (idx, result)
    };
    let mut results: Vec<(usize, FileResult)> = if opts.parallel && total > 1 {
        files.par_iter().enumerate().map(|(idx, path)| process(path, idx)).collect()
    } else {
        files.iter().enumerate().map(|(idx, path)| process(path, idx)).collect()
    };
    results.sort_by_key(|(idx, _)| *idx);
    BatchReport { results: results.into_iter().map(|(_, r)| r).collect() }
}
fn resolve_sources<'a>(
    layout: &[ChannelSource; 4],
    outs: &'a MapOutputs,
) -> Result<([ResolvedSource<'a>; 4], (u32, u32)), StepFailure> {
    let mut resolved: Vec<ResolvedSource<'a>> = Vec::with_capacity(4);
    let mut dims: [Option<(u32, u32)>; 4] = [None; 4];
    for (c, src) in layout.iter().enumerate() {
        match src {
            ChannelSource::Empty => resolved.push(ResolvedSource::Constant(0)),
            ChannelSource::Constant(v) => resolved.push(ResolvedSource::Constant(*v)),
            ChannelSource::Custom => return Err(StepFailure::UnsupportedChannel(PACK_CHANNELS[c])),
            ChannelSource::MapOutput(m) => {
                let g_owned = match m {
                    MapOutput::Roughness => outs.roughness.as_ref().cloned(),
                    MapOutput::Ao => outs.ao.as_ref().cloned(),
                    MapOutput::Height => outs.height.as_ref().cloned(),
                    MapOutput::Normal => outs.normal.as_ref().map(|n| GrayF32::from_rgba_channel(n, 2)),
                };
                match g_owned {
                    Some(g) => {
                        dims[c] = Some((g.width, g.height));
                        resolved.push(ResolvedSource::OwnedGray(g));
                    }
                    None => {
                        let key = match m {
                            MapOutput::Roughness => "maps-map-roughness",
                            MapOutput::Ao => "maps-map-ao",
                            MapOutput::Height => "maps-map-height",
                            MapOutput::Normal => "maps-map-normal",
                        };
                        return Err(StepFailure::MapMissing(key));
                    }
                }
            }
        }
    }
    let mut size: Option<(u32, u32)> = None;
    for (c, d) in dims.iter().enumerate() {
        if let Some(d) = *d {
            match size {
                None => size = Some(d),
                Some(s) if s != d => {
                    return Err(StepFailure::SizeMismatch(format!(
                        "channel {} is {}×{} but others are {}×{}",
                        PACK_CHANNELS[c], d.0, d.1, s.0, s.1
                    )));
                }
                _ => {}
            }
        }
    }
    let arr = [resolved.remove(0), resolved.remove(0), resolved.remove(0), resolved.remove(0)];
    Ok((arr, size.unwrap_or((1, 1))))
}
pub fn process_file(
    input: &Path,
    out_root: &Path,
    rel_dir: &Path,
    steps: &[BatchStep],
) -> FileResult {
    let rel_path = rel_dir
        .join(input.file_name().unwrap_or_default())
        .to_string_lossy()
        .replace('\\', "/");
    let mut current = match io::load(input) {
        Ok(img) => img,
        Err(e) => return FileResult { rel_path, outcome: FileOutcome::LoadFailed(e.to_string()) },
    };
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled".to_string());
    let out_dir = out_root.join(rel_dir);
    let mut outputs: Vec<PathBuf> = Vec::new();
    let mut maps_out: Option<MapOutputs> = None;
    for (i, step) in steps.iter().enumerate() {
        let step_failed =
            |reason: StepFailure| FileOutcome::StepFailed { index: i, step: step.name_key(), reason };
        match step {
            BatchStep::Tileable(mode) => {
                current = match mode {
                    TileableStepMode::Offset => tileable::offset_wrap(&current),
                    TileableStepMode::Mirror => tileable::mirror_tile(&current),
                };
            }
            BatchStep::Maps(params) => {
                let outs = maps::generate_all(&current, params);
                if outs.generated_count() == 0 {
                    return FileResult { rel_path, outcome: step_failed(StepFailure::NoMapsEnabled) };
                }
                let mut save = |name: &str, img: &RgbaImage| -> Result<(), CoreError> {
                    let p = out_dir.join(format!("{stem}_{name}.png"));
                    io::save(&p, img)?;
                    outputs.push(p);
                    Ok(())
                };
                if let Some(h) = &outs.height {
                    if let Err(e) = save("height", &h.to_rgba()) {
                        return FileResult { rel_path, outcome: FileOutcome::SaveFailed(e.to_string()) };
                    }
                }
                if let Some(n) = &outs.normal {
                    if let Err(e) = save("normal", n) {
                        return FileResult { rel_path, outcome: FileOutcome::SaveFailed(e.to_string()) };
                    }
                }
                if let Some(r) = &outs.roughness {
                    if let Err(e) = save("roughness", &r.to_rgba()) {
                        return FileResult { rel_path, outcome: FileOutcome::SaveFailed(e.to_string()) };
                    }
                }
                if let Some(a) = &outs.ao {
                    if let Err(e) = save("ao", &a.to_rgba()) {
                        return FileResult { rel_path, outcome: FileOutcome::SaveFailed(e.to_string()) };
                    }
                }
                maps_out = Some(outs);
            }
            BatchStep::Pack(preset) => {
                let Some(outs) = &maps_out else {
                    return FileResult { rel_path, outcome: step_failed(StepFailure::PackWithoutMaps) };
                };
                let layout = packing::preset_mapping(*preset);
                let (sources, (w, h)) = match resolve_sources(&layout, outs) {
                    Ok(v) => v,
                    Err(reason) => return FileResult { rel_path, outcome: step_failed(reason) },
                };
                let packed = packing::pack(w, h, &sources);
                let p = out_dir.join(format!("{stem}_packed.png"));
                if let Err(e) = io::save(&p, &packed) {
                    return FileResult { rel_path, outcome: FileOutcome::SaveFailed(e.to_string()) };
                }
                outputs.push(p);
                current = packed;
            }
            BatchStep::Optimize(params) => {
                current = optimize::resize(&current, &params.resize);
                let (ext, format) = match params.format {
                    BatchFormat::Png => ("png", ImageFormat::Png),
                    BatchFormat::Tga => ("tga", ImageFormat::Tga),
                    BatchFormat::Dds => ("dds", ImageFormat::Dds),
                };
                let p = out_dir.join(format!("{stem}.{ext}"));
                let res = if params.format == BatchFormat::Dds {
                    optimize::encode_dds(&current, params.bc, true)
                        .and_then(|bytes| std::fs::write(&p, bytes).map_err(|e| CoreError::Write(e.to_string())))
                } else {
                    io::save_with(&p, &current, format, None)
                };
                if let Err(e) = res {
                    return FileResult { rel_path, outcome: FileOutcome::SaveFailed(e.to_string()) };
                }
                outputs.push(p);
            }
        }
    }
    if outputs.is_empty() {
        let p = out_dir.join(format!("{stem}.png"));
        if let Err(e) = io::save(&p, &current) {
            return FileResult { rel_path, outcome: FileOutcome::SaveFailed(e.to_string()) };
        }
        outputs.push(p);
    }
    FileResult { rel_path, outcome: FileOutcome::Done { outputs } }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn unique_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tbx-batch-tests-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn cleanup(dirs: &[&PathBuf]) {
        for d in dirs {
            let _ = std::fs::remove_dir_all(d);
        }
    }
    fn write_test_image(dir: &Path, name: &str, size: u32) -> PathBuf {
        let p = dir.join(name);
        let mut img = RgbaImage::new(size, size);
        for y in 0..size {
            for x in 0..size {
                let v = ((x * 255) / size.max(1)) as u8;
                img.put_pixel(x, y, image::Rgba([v, v / 2, 255 - v, 255]));
            }
        }
        io::save(&p, &img).unwrap();
        p
    }
    #[test]
    fn full_chain_writes_expected_outputs() {
        let in_dir = unique_dir("full-in");
        let out_dir = unique_dir("full-out");
        write_test_image(&in_dir, "a.png", 64);
        let sub = in_dir.join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        write_test_image(&sub, "b.png", 64);
        let steps = vec![
            BatchStep::Tileable(TileableStepMode::Offset),
            BatchStep::Maps(MapSetParams::default()),
            BatchStep::Pack(EnginePreset::Godot),
            BatchStep::Optimize(OptimizeStepParams::default()),
        ];
        let report = run_batch(
            &in_dir,
            &out_dir,
            &steps,
            &BatchOptions { max_files: None, dry_run: false, parallel: false },
            &|_, _, r| assert!(r.ok(), "{:?} failed: {:?}", r.rel_path, r.outcome),
        );
        assert_eq!(report.ok_count(), 2);
        assert!(out_dir.join("a_height.png").exists());
        assert!(out_dir.join("a_packed.png").exists());
        assert!(out_dir.join("a.png").exists());
        assert!(out_dir.join("nested/b_normal.png").exists());
        assert!(out_dir.join("nested/b.png").exists());
        cleanup(&[&in_dir, &out_dir]);
    }
    #[test]
    fn tileable_only_chain_writes_main_image() {
        let in_dir = unique_dir("tile-in");
        let out_dir = unique_dir("tile-out");
        write_test_image(&in_dir, "x.png", 32);
        let steps = vec![BatchStep::Tileable(TileableStepMode::Mirror)];
        let report = run_batch(
            &in_dir,
            &out_dir,
            &steps,
            &BatchOptions { max_files: None, dry_run: false, parallel: false },
            &|_, _, _| {},
        );
        assert_eq!(report.ok_count(), 1);
        assert!(out_dir.join("x.png").exists());
        cleanup(&[&in_dir, &out_dir]);
    }
    #[test]
    fn pack_without_maps_fails() {
        let in_dir = unique_dir("packbad-in");
        let out_dir = unique_dir("packbad-out");
        write_test_image(&in_dir, "x.png", 32);
        let steps = vec![BatchStep::Pack(EnginePreset::UnrealOrm)];
        let report = run_batch(
            &in_dir,
            &out_dir,
            &steps,
            &BatchOptions { max_files: None, dry_run: false, parallel: false },
            &|_, _, _| {},
        );
        assert_eq!(report.ok_count(), 0);
        assert!(matches!(
            report.results[0].outcome,
            FileOutcome::StepFailed { reason: StepFailure::PackWithoutMaps, .. }
        ));
        assert_eq!(validate_chain(&steps), Some(0));
        cleanup(&[&in_dir, &out_dir]);
    }
    #[test]
    fn dry_run_processes_first_file_into_dryrun_dir() {
        let in_dir = unique_dir("dry-in");
        let out_dir = unique_dir("dry-out");
        write_test_image(&in_dir, "a.png", 32);
        write_test_image(&in_dir, "b.png", 32);
        let steps = vec![BatchStep::Optimize(OptimizeStepParams::default())];
        let report = run_batch(
            &in_dir,
            &out_dir,
            &steps,
            &BatchOptions { max_files: None, dry_run: true, parallel: false },
            &|_, _, _| {},
        );
        assert_eq!(report.results.len(), 1);
        assert!(out_dir.join("dry-run/a.png").exists());
        assert!(!out_dir.join("b.png").exists());
        cleanup(&[&in_dir, &out_dir]);
    }
    #[test]
    fn collect_inputs_ignores_foreign_files_and_respects_max() {
        let in_dir = unique_dir("collect-in");
        write_test_image(&in_dir, "a.png", 16);
        std::fs::write(in_dir.join("b.jpg"), b"not really a jpeg").unwrap();
        std::fs::write(in_dir.join("notes.txt"), "not an image").unwrap();
        let files = collect_inputs(&in_dir);
        assert_eq!(files.len(), 2);
        let out_dir = unique_dir("collect-out");
        let steps = vec![BatchStep::Optimize(OptimizeStepParams::default())];
        let report = run_batch(
            &in_dir,
            &out_dir,
            &steps,
            &BatchOptions { max_files: Some(1), dry_run: false, parallel: false },
            &|_, _, _| {},
        );
        assert_eq!(report.results.len(), 1);
        cleanup(&[&in_dir, &out_dir]);
    }
}
