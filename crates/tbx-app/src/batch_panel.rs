use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use fluent_bundle::FluentValue;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use tbx_core::batch::{
    self, BatchFormat, BatchOptions, BatchStep, FileOutcome, FileResult, OptimizeStepParams,
    StepFailure, TileableStepMode,
};
use tbx_core::maps::MapSetParams;
use tbx_core::optimize::{DdsCompression, OptimizeParams, PotSnap, Resampling};
use tbx_core::packing::EnginePreset;
use tbx_entitlements::Capability;
use tbx_locale::LocaleManager;
use tbx_state::AppState;
use crate::{BatchApi, BatchLogLine, MainWindow};
const FREE_MAX_FILES: usize = 25;
const OPT_SIZES: [Option<u32>; 8] = [
    None,
    Some(64),
    Some(128),
    Some(256),
    Some(512),
    Some(1024),
    Some(2048),
    Some(4096),
];
struct BatchWork {
    steps: Vec<BatchStep>,
    selected: Option<usize>,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    running: bool,
}
fn default_map_params(_state: &AppState) -> MapSetParams {
    MapSetParams::default()
}
fn clamp_chain_for_plan(steps: &mut [BatchStep], state: &AppState) {
    for step in steps.iter_mut() {
        match step {
            BatchStep::Optimize(opt)
                if opt.format == BatchFormat::Dds
                    && state.gate.is_locked(Capability::OptimizeDdsCompression)
                => {
                    opt.format = BatchFormat::Png;
                }
            _ => {}
        }
    }
}
fn step_display(step: &BatchStep, locale: &LocaleManager) -> String {
    match step {
        BatchStep::Tileable(mode) => {
            let mode_key = match mode {
                TileableStepMode::Offset => "batch-mode-offset",
                TileableStepMode::Mirror => "batch-mode-mirror",
            };
            format!("{} — {}", locale.tr("batch-step-tileable"), locale.tr(mode_key))
        }
        BatchStep::Maps(_) => locale.tr("batch-step-maps"),
        BatchStep::Pack(preset) => {
            let key = match preset {
                EnginePreset::Custom => "pack-preset-custom",
                EnginePreset::UnrealOrm => "pack-preset-unreal",
                EnginePreset::UnityMetallic => "pack-preset-unity",
                EnginePreset::Godot => "pack-preset-godot",
            };
            format!("{} — {}", locale.tr("batch-step-pack"), locale.tr(key))
        }
        BatchStep::Optimize(opt) => {
            let size = match opt.resize.target_size {
                None => locale.tr("batch-size-original"),
                Some(s) => s.to_string(),
            };
            let fmt_key = match opt.format {
                BatchFormat::Png => "optimize-format-png",
                BatchFormat::Tga => "optimize-format-tga",
                BatchFormat::Dds => "optimize-format-dds",
            };
            let detail = locale.tr_args(
                "batch-step-detail-optimize",
                &[("size", size.as_str().into()), ("format", locale.tr(fmt_key).as_str().into())],
            );
            format!("{} — {}", locale.tr("batch-step-optimize"), detail)
        }
    }
}
fn refresh_chain(
    window: &MainWindow,
    work: &mut BatchWork,
    locale: &LocaleManager,
    chain_locked: bool,
) {
    let api = window.global::<BatchApi>();
    let names: Vec<slint::SharedString> =
        work.steps.iter().map(|s| step_display(s, locale).into()).collect();
    api.set_steps(ModelRc::new(VecModel::from(names)));
    api.set_selected_step(work.selected.map(|i| i as i32).unwrap_or(-1));
    api.set_selected_kind(match work.selected.and_then(|i| work.steps.get(i)) {
        Some(BatchStep::Tileable(_)) => 0,
        Some(BatchStep::Maps(_)) => 1,
        Some(BatchStep::Pack(_)) => 2,
        Some(BatchStep::Optimize(_)) => 3,
        None => -1,
    });
    api.set_chain_full(chain_locked && !work.steps.is_empty());
}
fn push_step_params(window: &MainWindow, work: &mut BatchWork) {
    let Some(step) = work.selected.and_then(|i| work.steps.get(i).cloned()) else { return };
    let api = window.global::<BatchApi>();
    match step {
        BatchStep::Tileable(mode) => {
            api.set_tileable_mode(if mode == TileableStepMode::Mirror { 1 } else { 0 });
        }
        BatchStep::Pack(preset) => {
            let i = match preset {
                EnginePreset::UnityMetallic => 1,
                EnginePreset::Godot => 2,
                _ => 0,
            };
            api.set_pack_preset(i);
        }
        BatchStep::Optimize(opt) => {
            let size_idx = OPT_SIZES.iter().position(|s| *s == opt.resize.target_size).unwrap_or(0);
            let fmt_idx = match opt.format {
                BatchFormat::Png => 0,
                BatchFormat::Tga => 1,
                BatchFormat::Dds => 2,
            };
            let bc_idx = match opt.bc {
                DdsCompression::Bc1 => 0,
                DdsCompression::Bc3 => 1,
                DdsCompression::Bc5 => 2,
                DdsCompression::Bc7 => 3,
            };
            api.set_opt_size(size_idx as i32);
            api.set_opt_format(fmt_idx);
            api.set_opt_bc(bc_idx);
        }
        BatchStep::Maps(_) => {}
    }
}
fn format_log_line(locale: &LocaleManager, result: &FileResult) -> (i32, String) {
    let path: FluentValue<'_> = result.rel_path.as_str().into();
    match &result.outcome {
        FileOutcome::Done { outputs } => (1, locale.tr_args(
            "batch-log-ok",
            &[("path", path), ("count", (outputs.len() as i64).into())],
        )),
        FileOutcome::LoadFailed(_) => (3, locale.tr_args("batch-log-load-fail", &[("path", path)])),
        FileOutcome::SaveFailed(e) => (3,
            locale.tr_args("batch-log-save-fail", &[("path", path), ("error", e.as_str().into())])
        ),
        FileOutcome::StepFailed { index, step, reason } => {
            let reason_text = match reason {
                StepFailure::NoMapsEnabled => locale.tr("batch-step-reason-no-maps"),
                StepFailure::PackWithoutMaps => locale.tr("batch-step-reason-pack-no-maps"),
                StepFailure::MapMissing(key) => locale.tr_args(
                    "batch-step-reason-map-missing",
                    &[("map", locale.tr(key).as_str().into())],
                ),
                StepFailure::UnsupportedChannel(c) => locale.tr_args(
                    "batch-step-reason-custom-channel",
                    &[("channel", c.to_string().as_str().into())],
                ),
                StepFailure::SizeMismatch(e) => locale
                    .tr_args("batch-step-reason-size-mismatch", &[("error", e.as_str().into())]),
            };
            (2, locale.tr_args(
                "batch-log-step-fail",
                &[
                    ("path", path),
                    ("index", ((*index + 1) as i64).into()),
                    ("step", locale.tr(step).as_str().into()),
                    ("reason", reason_text.as_str().into()),
                ],
            ))
        }
    }
}
fn start_run(
    window_weak: slint::Weak<MainWindow>,
    state: &Arc<AppState>,
    locale: &Arc<LocaleManager>,
    work: &Arc<Mutex<BatchWork>>,
    dry_run: bool,
) {
    let Some(window) = window_weak.upgrade() else { return };
    let api = window.global::<BatchApi>();
    if api.get_running() {
        return;
    }
    if dry_run && state.gate.is_locked(Capability::BatchDryRun) {
        api.set_status(locale.tr("batch-locked-dry-run").into());
        return;
    }
    let (steps, input, output) = {
        let w = work.lock().unwrap_or_else(|e| e.into_inner());
        if w.running {
            return;
        }
        (w.steps.clone(), w.input.clone(), w.output.clone())
    };
    if steps.is_empty() {
        api.set_status(locale.tr("batch-status-no-steps").into());
        return;
    }
    if batch::validate_chain(&steps).is_some() {
        api.set_status(locale.tr("batch-status-pack-needs-maps").into());
        return;
    }
    let (Some(input), Some(output)) = (input, output) else {
        api.set_status(locale.tr("batch-status-no-folders").into());
        return;
    };
    {
        let in_c = input.canonicalize().unwrap_or_else(|_| input.clone());
        let out_c = output.canonicalize().unwrap_or_else(|_| output.clone());
        if in_c == out_c || in_c.starts_with(&out_c) || out_c.starts_with(&in_c) {
            api.set_status(locale.tr("batch-status-same-folder").into());
            return;
        }
    }
    let all_files = batch::collect_inputs(&input);
    if all_files.is_empty() {
        api.set_status(locale.tr("batch-status-no-files").into());
        return;
    }
    let chain_locked = state.gate.is_locked(Capability::BatchChaining);
    let mut steps = steps;
    if chain_locked {
        steps.truncate(1);
    }
    clamp_chain_for_plan(&mut steps, state);
    let max_files = if state.gate.is_locked(Capability::BatchUnlimitedFiles) {
        Some(FREE_MAX_FILES)
    } else {
        None
    };
    let parallel = !state.gate.is_locked(Capability::BatchScheduledProcessing);
    let opts = BatchOptions { max_files, dry_run, parallel };
    if max_files.is_some() && all_files.len() > FREE_MAX_FILES {
        api.set_status(
            locale
                .tr_args("batch-status-capped-run", &[("max", (FREE_MAX_FILES as i64).into())])
                .into(),
        );
    }
    work.lock().unwrap_or_else(|e| e.into_inner()).running = true;
    api.set_running(true);
    api.set_progress(0.0);
    api.set_log(ModelRc::new(VecModel::default()));
    let t0 = Instant::now();
    let locale_run = locale.clone();
    let work_run = work.clone();
    let weak_cb = window_weak.clone();
    std::thread::spawn(move || {
        let progress_locale = locale_run.clone();
        let report = batch::run_batch(&input, &output, &steps, &opts, &|done, total, result| {
            let (level, text) = format_log_line(&progress_locale, result);
            let entry = BatchLogLine { level, text: text.into() };
            let progress_weak = weak_cb.clone();
            let progress_locale = progress_locale.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(window) = progress_weak.upgrade() else { return };
                let api = window.global::<BatchApi>();
                if total > 0 {
                    api.set_progress(done as f32 / total as f32);
                }
                api.set_status(
                    progress_locale
                        .tr_args(
                            "batch-status-running",
                            &[("done", (done as i64).into()), ("total", (total as i64).into())],
                        )
                        .into(),
                );
                let mut lines: Vec<BatchLogLine> = api.get_log().iter().collect();
                lines.push(entry);
                api.set_log(ModelRc::new(VecModel::from(lines)));
            });
        });
        let secs = t0.elapsed().as_secs_f64();
        let ok = report.ok_count();
        let total = report.results.len();
        let _ = slint::invoke_from_event_loop(move || {
            let Some(window) = weak_cb.upgrade() else { return };
            let api = window.global::<BatchApi>();
            work_run.lock().unwrap_or_else(|e| e.into_inner()).running = false;
            api.set_running(false);
            api.set_progress(1.0);
            let status = if dry_run {
                locale_run.tr("batch-status-dry-done")
            } else {
                locale_run.tr_args(
                    "batch-status-done",
                    &[
                        ("ok", (ok as i64).into()),
                        ("total", (total as i64).into()),
                        ("secs", FluentValue::from(format!("{:.2}", secs))),
                    ],
                )
            };
            api.set_status(status.into());
        });
    });
}
fn persist_chain(state: &AppState, steps: &[BatchStep]) {
    state.project.write().unwrap_or_else(|e| e.into_inner()).batch_chain = steps.to_vec();
}
pub fn sync_entitlements(window: &MainWindow, state: &AppState) {
    let gate = &state.gate;
    let api = window.global::<BatchApi>();
    api.set_lock_chain(gate.is_locked(Capability::BatchChaining));
    api.set_lock_files(gate.is_locked(Capability::BatchUnlimitedFiles));
    api.set_lock_dry_run(gate.is_locked(Capability::BatchDryRun));
    api.set_lock_parallel(gate.is_locked(Capability::BatchScheduledProcessing));
    api.set_lock_pack_step(gate.is_locked(Capability::ChannelPackEnginePresets));
    api.set_lock_dds(gate.is_locked(Capability::OptimizeDdsCompression));
}
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let work: Arc<Mutex<BatchWork>> = Arc::new(Mutex::new(BatchWork {
        steps: state.project.read().unwrap_or_else(|e| e.into_inner()).batch_chain.clone(),
        selected: None,
        input: None,
        output: None,
        running: false,
    }));
    window.global::<BatchApi>().on_add_step({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<BatchApi>();
            let kind = api.get_add_kind();
            let chain_locked = state.gate.is_locked(Capability::BatchChaining);
            if kind == 2 && state.gate.is_locked(Capability::ChannelPackEnginePresets) {
                api.set_status(locale.tr("batch-locked-pack-step").into());
                return;
            }
            let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
            if chain_locked && !w.steps.is_empty() {
                api.set_status(locale.tr("batch-locked-chain").into());
                return;
            }
            let step = match kind {
                0 => BatchStep::Tileable(TileableStepMode::Offset),
                1 => BatchStep::Maps(default_map_params(&state)),
                2 => BatchStep::Pack(EnginePreset::UnrealOrm),
                _ => BatchStep::Optimize(OptimizeStepParams::default()),
            };
            w.steps.push(step);
            w.selected = Some(w.steps.len() - 1);
            persist_chain(&state, &w.steps);
            refresh_chain(&window, &mut w, &locale, chain_locked);
            push_step_params(&window, &mut w);
        }
    });
    window.global::<BatchApi>().on_select_step({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move |i| {
            let Some(window) = weak.upgrade() else { return };
            let chain_locked = state.gate.is_locked(Capability::BatchChaining);
            let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
            if i >= 0 && (i as usize) < w.steps.len() {
                w.selected = Some(i as usize);
            }
            refresh_chain(&window, &mut w, &locale, chain_locked);
            push_step_params(&window, &mut w);
        }
    });
    window.global::<BatchApi>().on_remove_step({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let chain_locked = state.gate.is_locked(Capability::BatchChaining);
            let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(i) = w.selected {
                if i < w.steps.len() {
                    w.steps.remove(i);
                    w.selected =
                        if w.steps.is_empty() { None } else { Some(i.min(w.steps.len() - 1)) };
                }
            }
            persist_chain(&state, &w.steps);
            refresh_chain(&window, &mut w, &locale, chain_locked);
        }
    });
    window.global::<BatchApi>().on_move_selected({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move |dir| {
            let Some(window) = weak.upgrade() else { return };
            let chain_locked = state.gate.is_locked(Capability::BatchChaining);
            let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(i) = w.selected {
                let j = i as isize + dir as isize;
                if j >= 0 && (j as usize) < w.steps.len() {
                    w.steps.swap(i, j as usize);
                    w.selected = Some(j as usize);
                }
            }
            persist_chain(&state, &w.steps);
            refresh_chain(&window, &mut w, &locale, chain_locked);
        }
    });
    window.global::<BatchApi>().on_update_tileable({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<BatchApi>();
            let mode = if api.get_tileable_mode() == 1 {
                TileableStepMode::Mirror
            } else {
                TileableStepMode::Offset
            };
            let chain_locked = state.gate.is_locked(Capability::BatchChaining);
            let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(i) = w.selected {
                if let Some(BatchStep::Tileable(m)) = w.steps.get_mut(i) {
                    *m = mode;
                }
            }
            persist_chain(&state, &w.steps);
            refresh_chain(&window, &mut w, &locale, chain_locked);
        }
    });
    window.global::<BatchApi>().on_update_pack({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<BatchApi>();
            let preset = match api.get_pack_preset() {
                1 => EnginePreset::UnityMetallic,
                2 => EnginePreset::Godot,
                _ => EnginePreset::UnrealOrm,
            };
            let chain_locked = state.gate.is_locked(Capability::BatchChaining);
            let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(i) = w.selected {
                if let Some(BatchStep::Pack(p)) = w.steps.get_mut(i) {
                    *p = preset;
                }
            }
            persist_chain(&state, &w.steps);
            refresh_chain(&window, &mut w, &locale, chain_locked);
        }
    });
    window.global::<BatchApi>().on_update_opt({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<BatchApi>();
            let size_idx = api.get_opt_size().clamp(0, OPT_SIZES.len() as i32 - 1) as usize;
            let format = match api.get_opt_format() {
                1 => BatchFormat::Tga,
                2 => BatchFormat::Dds,
                _ => BatchFormat::Png,
            };
            let format = if format == BatchFormat::Dds
                && state.gate.is_locked(Capability::OptimizeDdsCompression)
            {
                api.set_opt_format(0);
                api.set_status(locale.tr("optimize-locked-dds").into());
                BatchFormat::Png
            } else {
                format
            };
            let bc = match api.get_opt_bc() {
                0 => DdsCompression::Bc1,
                2 => DdsCompression::Bc5,
                3 => DdsCompression::Bc7,
                _ => DdsCompression::Bc3,
            };
            let params = OptimizeStepParams {
                resize: OptimizeParams {
                    target_size: OPT_SIZES[size_idx],
                    pot_snap: PotSnap::None,
                    resampling: Resampling::Lanczos3,
                    dds: None,
                },
                format,
                bc,
            };
            let chain_locked = state.gate.is_locked(Capability::BatchChaining);
            let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(i) = w.selected {
                if let Some(BatchStep::Optimize(o)) = w.steps.get_mut(i) {
                    *o = params;
                }
            }
            persist_chain(&state, &w.steps);
            refresh_chain(&window, &mut w, &locale, chain_locked);
        }
    });
    window.global::<BatchApi>().on_browse_input({
        let weak = window.as_weak().clone();
        let locale = locale.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<BatchApi>();
            let title = locale.tr("batch-input-folder");
            let Some(dir) = rfd::FileDialog::new().set_title(&title).pick_folder() else { return };
            let count = batch::collect_inputs(&dir).len();
            api.set_input_folder(dir.to_string_lossy().as_ref().into());
            api.set_files_label(
                locale.tr_args("batch-files-found", &[("count", (count as i64).into())]).into(),
            );
            work.lock().unwrap_or_else(|e| e.into_inner()).input = Some(dir);
        }
    });
    window.global::<BatchApi>().on_browse_output({
        let weak = window.as_weak().clone();
        let locale = locale.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<BatchApi>();
            let title = locale.tr("batch-output-folder");
            let Some(dir) = rfd::FileDialog::new().set_title(&title).pick_folder() else { return };
            api.set_output_folder(dir.to_string_lossy().as_ref().into());
            work.lock().unwrap_or_else(|e| e.into_inner()).output = Some(dir);
        }
    });
    window.global::<BatchApi>().on_dry_run({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move || start_run(weak.clone(), &state, &locale, &work, true)
    });
    window.global::<BatchApi>().on_run_batch({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move || start_run(weak.clone(), &state, &locale, &work, false)
    });
    window.global::<BatchApi>().on_reload({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let chain_locked = state.gate.is_locked(Capability::BatchChaining);
            let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
            w.steps = state.project.read().unwrap_or_else(|e| e.into_inner()).batch_chain.clone();
            w.selected = if w.steps.is_empty() { None } else { Some(0) };
            refresh_chain(&window, &mut w, &locale, chain_locked);
            push_step_params(&window, &mut w);
        }
    });
    {
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let work = work.clone();
        let rx = state.bus.subscribe();
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                if !matches!(event, tbx_state::AppEvent::LanguageChanged) {
                    continue;
                }
                let _ = slint::invoke_from_event_loop({
                    let weak = weak.clone();
                    let state = state.clone();
                    let locale = locale.clone();
                    let work = work.clone();
                    move || {
                        let Some(window) = weak.upgrade() else { return };
                        let chain_locked = state.gate.is_locked(Capability::BatchChaining);
                        let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
                        refresh_chain(&window, &mut w, &locale, chain_locked);
                    }
                });
            }
        });
    }
}
