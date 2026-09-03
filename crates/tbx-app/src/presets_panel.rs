use std::sync::{Arc, Mutex};
use slint::{ComponentHandle, ModelRc, VecModel};
use tbx_core::maps::{MapSetParams, NormalKernel};
use tbx_core::packing::{ChannelSource, MapOutput};
use tbx_entitlements::Capability;
use tbx_locale::LocaleManager;
use tbx_state::AppState;
use crate::presets::{
    export_text, import_text, load_all, save_all, sanitize_name, AtlasPreset, Feature,
    OptimizePreset, PackingPreset, Preset, PresetPayload, TileablePreset, FREE_MAX_PER_FEATURE,
};
use crate::{AtlasApi, BatchApi, MainWindow, MapsApi, Nav, OptimizeApi, PackingApi, PresetsApi, TileableApi};
type PresetStore = Arc<Mutex<Vec<Preset>>>;
fn sel_idx(i: i32) -> Option<usize> {
    (i >= 0).then_some(i as usize)
}
const SAVE_FEATURES: [Feature; 7] = [
    Feature::Maps,
    Feature::Tileable,
    Feature::Packing,
    Feature::Atlas,
    Feature::Optimize,
    Feature::Batch,
    Feature::Project,
];
fn index_to_source(i: i32) -> ChannelSource {
    match i {
        1 => ChannelSource::Constant(0),
        2 => ChannelSource::Constant(255),
        3 => ChannelSource::MapOutput(MapOutput::Roughness),
        4 => ChannelSource::MapOutput(MapOutput::Ao),
        5 => ChannelSource::MapOutput(MapOutput::Height),
        6 => ChannelSource::Custom,
        _ => ChannelSource::Empty,
    }
}
fn source_to_index(src: &ChannelSource) -> i32 {
    match src {
        ChannelSource::Empty => 0,
        ChannelSource::Constant(v) if *v >= 128 => 2,
        ChannelSource::Constant(_) => 1,
        ChannelSource::MapOutput(MapOutput::Roughness) => 3,
        ChannelSource::MapOutput(MapOutput::Ao) => 4,
        ChannelSource::MapOutput(MapOutput::Height) => 5,
        ChannelSource::MapOutput(MapOutput::Normal) => 7,
        ChannelSource::Custom => 6,
    }
}
fn capture_maps(window: &MainWindow, _state: &AppState) -> MapSetParams {
    let api = window.global::<MapsApi>();
    MapSetParams {
        height: tbx_core::maps::HeightParams {
            source: match api.get_height_source_channel() {
                1 => tbx_core::maps::HeightSource::Red,
                2 => tbx_core::maps::HeightSource::Green,
                3 => tbx_core::maps::HeightSource::Blue,
                4 => tbx_core::maps::HeightSource::Alpha,
                _ => tbx_core::maps::HeightSource::Luminance,
            },
            contrast: api.get_contrast(),
            brightness: api.get_brightness(),
            blur_radius: api.get_blur_radius(),
            invert: api.get_invert_height(),
        },
        normal: tbx_core::maps::NormalParams {
            strength: api.get_normal_strength(),
            auto_strength: api.get_normal_auto_strength(),
            detail_enhance: api.get_detail_enhance(),
            kernel: if api.get_normal_kernel() == 0 { NormalKernel::K3 } else { NormalKernel::K5 },
            wrap_edges: api.get_wrap_edges(),
        },
        roughness: tbx_core::maps::RoughnessParams {
            radius: api.get_rough_radius(),
            amount: api.get_rough_amount(),
            curve: api.get_rough_curve(),
            invert: api.get_rough_invert(),
        },
        ao: tbx_core::maps::AoParams {
            radius: api.get_ao_radius(),
            intensity: api.get_ao_intensity(),
        },
        enable_height: api.get_enable_height(),
        enable_normal: api.get_enable_normal(),
        enable_roughness: api.get_enable_roughness(),
        enable_ao: api.get_enable_ao(),
    }
}
fn capture_tileable(window: &MainWindow) -> TileablePreset {
    let api = window.global::<TileableApi>();
    TileablePreset {
        heal_strength: api.get_heal_strength(),
        heal_radius: api.get_heal_radius(),
        heal_passes: api.get_heal_passes().round() as u8,
        mirror_mode: api.get_mirror_mode(),
        brush_size: api.get_brush_size(),
        brush_offset_x: api.get_brush_offset_x(),
        brush_offset_y: api.get_brush_offset_y(),
    }
}
fn capture_packing(window: &MainWindow) -> PackingPreset {
    let api = window.global::<PackingApi>();
    PackingPreset {
        channels: [
            index_to_source(api.get_source_r()),
            index_to_source(api.get_source_g()),
            index_to_source(api.get_source_b()),
            index_to_source(api.get_source_a()),
        ],
    }
}
fn capture_atlas(window: &MainWindow) -> AtlasPreset {
    let api = window.global::<AtlasApi>();
    AtlasPreset {
        size_index: api.get_size_index(),
        padding: api.get_padding().round() as u32,
        bleed: api.get_bleed().round() as u32,
        rotation: api.get_allow_rotation(),
        trim: api.get_trim_sheet(),
        sidecar: api.get_sidecar_index(),
    }
}
fn capture_optimize(window: &MainWindow) -> OptimizePreset {
    let api = window.global::<OptimizeApi>();
    OptimizePreset {
        size_index: api.get_size_index(),
        snap_index: api.get_snap_index(),
        resampling_index: api.get_resampling_index(),
    }
}
fn capture_payload(
    window: &MainWindow,
    state: &AppState,
    feature: Feature,
) -> Option<PresetPayload> {
    match feature {
        Feature::Maps => Some(PresetPayload::Maps(capture_maps(window, state))),
        Feature::Tileable => Some(PresetPayload::Tileable(capture_tileable(window))),
        Feature::Packing => Some(PresetPayload::Packing(capture_packing(window))),
        Feature::Atlas => Some(PresetPayload::Atlas(capture_atlas(window))),
        Feature::Optimize => Some(PresetPayload::Optimize(capture_optimize(window))),
        Feature::Batch => {
            let chain = state.project.read().unwrap_or_else(|e| e.into_inner()).batch_chain.clone();
            if chain.is_empty() {
                None
            } else {
                Some(PresetPayload::Batch { steps: chain })
            }
        }
        Feature::Project => {
            let mut entries: Vec<(String, PresetPayload)> = vec![
                ("maps".into(), PresetPayload::Maps(capture_maps(window, state))),
                ("tileable".into(), PresetPayload::Tileable(capture_tileable(window))),
                ("packing".into(), PresetPayload::Packing(capture_packing(window))),
                ("atlas".into(), PresetPayload::Atlas(capture_atlas(window))),
                ("optimize".into(), PresetPayload::Optimize(capture_optimize(window))),
            ];
            let chain = state.project.read().unwrap_or_else(|e| e.into_inner()).batch_chain.clone();
            if !chain.is_empty() {
                entries.push(("batch".into(), PresetPayload::Batch { steps: chain }));
            }
            Some(PresetPayload::Project { entries })
        }
    }
}
fn apply_payload(window: &MainWindow, state: &AppState, payload: &PresetPayload) {
    match payload {
        PresetPayload::Maps(p) => {
            let api = window.global::<MapsApi>();
            api.set_contrast(p.height.contrast);
            api.set_brightness(p.height.brightness);
            api.set_blur_radius(p.height.blur_radius);
            api.set_invert_height(p.height.invert);
            api.set_normal_strength(p.normal.strength);
            api.set_normal_kernel(if p.normal.kernel == NormalKernel::K3 { 0 } else { 1 });
            api.set_wrap_edges(p.normal.wrap_edges);
            api.set_rough_radius(p.roughness.radius);
            api.set_rough_amount(p.roughness.amount);
            api.set_rough_curve(p.roughness.curve);
            api.set_rough_invert(p.roughness.invert);
            api.set_ao_radius(p.ao.radius);
            api.set_ao_intensity(p.ao.intensity);
            api.set_enable_height(p.enable_height);
            api.set_enable_normal(p.enable_normal);
            api.set_enable_roughness(p.enable_roughness);
            api.set_enable_ao(p.enable_ao);
            state.project.write().unwrap_or_else(|e| e.into_inner()).maps.params = p.clone();
            api.invoke_generate();
        }
        PresetPayload::Tileable(t) => {
            let api = window.global::<TileableApi>();
            api.set_heal_strength(t.heal_strength);
            api.set_heal_radius(t.heal_radius);
            api.set_heal_passes(t.heal_passes as f32);
            api.set_mirror_mode(t.mirror_mode);
            api.set_brush_size(t.brush_size);
            api.set_brush_offset_x(t.brush_offset_x);
            api.set_brush_offset_y(t.brush_offset_y);
            if api.get_has_work() {
                api.invoke_reset();
            }
        }
        PresetPayload::Packing(p) => {
            let api = window.global::<PackingApi>();
            api.set_source_r(source_to_index(&p.channels[0]));
            api.set_source_g(source_to_index(&p.channels[1]));
            api.set_source_b(source_to_index(&p.channels[2]));
            api.set_source_a(source_to_index(&p.channels[3]));
        }
        PresetPayload::Atlas(a) => {
            let api = window.global::<AtlasApi>();
            api.set_size_index(a.size_index);
            api.set_padding(a.padding as f32);
            api.set_bleed(a.bleed as f32);
            api.set_allow_rotation(a.rotation);
            api.set_trim_sheet(a.trim);
            api.set_sidecar_index(a.sidecar);
        }
        PresetPayload::Optimize(o) => {
            let api = window.global::<OptimizeApi>();
            api.set_size_index(o.size_index);
            api.set_snap_index(o.snap_index);
            api.set_resampling_index(o.resampling_index);
        }
        PresetPayload::Batch { steps } => {
            state.project.write().unwrap_or_else(|e| e.into_inner()).batch_chain = steps.clone();
            window.global::<BatchApi>().invoke_reload();
        }
        PresetPayload::Project { entries } => {
            for (key, payload) in entries {
                if Feature::from_key(key).is_some() {
                    apply_payload(window, state, payload);
                }
            }
        }
    }
}
fn refresh_list(window: &MainWindow, store: &PresetStore, locale: &LocaleManager) {
    let api = window.global::<PresetsApi>();
    let mut presets = store.lock().unwrap_or_else(|e| e.into_inner());
    let selected_name = sel_idx(api.get_selected()).and_then(|i| presets.get(i).map(|p| p.name.clone()));
    presets.sort_by_key(|p| std::cmp::Reverse(p.last_used));
    let tag = locale.language();
    let rows: Vec<crate::PresetEntry> = presets
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            let when = chrono::DateTime::from_timestamp(p.last_used, 0)
                .map(|dt| tbx_locale::format_date(dt.date_naive(), &tag))
                .unwrap_or_default();
            let label: slint::SharedString = locale
                .tr_args(
                    "presets-list-item",
                    &[
                        ("name", p.name.as_str().into()),
                        ("feature", locale.tr(p.payload.feature().name_key()).as_str().into()),
                        ("when", when.as_str().into()),
                    ],
                )
                .into();
            let feature: slint::SharedString = match p.payload.feature().name_key() {
                "presets-feature-maps"     => "maps",
                "presets-feature-tileable" => "tileable",
                "presets-feature-packing"  => "packing",
                "presets-feature-atlas"    => "atlas",
                "presets-feature-optimize" => "optimize",
                "presets-feature-batch"    => "batch",
                "presets-feature-project"  => "project",
                _                          => "",
            }.into();
            crate::PresetEntry { index: idx as i32, label, feature }
        })
        .collect();
    api.set_preset_list(ModelRc::new(VecModel::from(rows)));
    api.set_selected(
        selected_name
            .as_deref()
            .and_then(|n| presets.iter().position(|p| p.name == n))
            .map(|i| i as i32)
            .unwrap_or(-1),
    );
}
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
pub fn sync_entitlements(window: &MainWindow, state: &AppState) {
    let gate = &state.gate;
    let api = window.global::<PresetsApi>();
    api.set_lock_unlimited(gate.is_locked(Capability::PresetsUnlimited));
    api.set_lock_export(gate.is_locked(Capability::PresetsExportImport));
}
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let store: PresetStore = Arc::new(Mutex::new(load_all()));
    refresh_list(window, &store, &locale);
    window.global::<PresetsApi>().on_save_preset({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let store = store.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PresetsApi>();
            let name = sanitize_name(api.get_preset_name().as_ref());
            if name.is_empty() {
                api.set_status(locale.tr("presets-status-no-name").into());
                return;
            }
            let idx = api.get_save_feature().clamp(0, SAVE_FEATURES.len() as i32 - 1) as usize;
            let feature = SAVE_FEATURES[idx];
            let Some(payload) = capture_payload(&window, &state, feature) else {
                api.set_status(locale.tr("presets-status-empty-chain").into());
                return;
            };
            {
                let presets = store.lock().unwrap_or_else(|e| e.into_inner());
                if presets.iter().any(|p| p.name == name) {
                    api.set_status(
                        locale.tr_args("presets-status-dup", &[("name", name.as_str().into())]).into(),
                    );
                    return;
                }
                if state.gate.is_locked(Capability::PresetsUnlimited) {
                    let count = presets.iter().filter(|p| p.payload.feature() == feature).count();
                    if count >= FREE_MAX_PER_FEATURE {
                        api.set_status(
                            locale
                                .tr_args(
                                    "presets-status-cap",
                                    &[("max", (FREE_MAX_PER_FEATURE as i64).into())],
                                )
                                .into(),
                        );
                        return;
                    }
                }
            }
            let preset = Preset { name: name.clone(), payload, last_used: now() };
            {
                let mut presets = store.lock().unwrap_or_else(|e| e.into_inner());
                presets.push(preset);
                if let Err(e) = save_all(&presets) {
                    presets.pop();
                    api.set_status(
                        locale
                            .tr_args("presets-status-export-error", &[("error", e.as_str().into())])
                            .into(),
                    );
                    return;
                }
            }
            refresh_list(&window, &store, &locale);
            api.set_preset_name("".into());
            api.set_status(
                locale.tr_args("presets-status-saved", &[("name", name.as_str().into())]).into(),
            );
        }
    });
    window.global::<PresetsApi>().on_apply_preset({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let store = store.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PresetsApi>();
            let Some(i) = sel_idx(api.get_selected()) else {
                api.set_status(locale.tr("presets-status-select-first").into());
                return;
            };
            let (payload, name) = {
                let mut presets = store.lock().unwrap_or_else(|e| e.into_inner());
                let Some(preset) = presets.get_mut(i) else { return };
                preset.last_used = now();
                let name = preset.name.clone();
                let payload = preset.payload.clone();
                if let Err(e) = save_all(&presets) {
                    if cfg!(debug_assertions) {
                        eprintln!("[texelbox] preset bookkeeping save failed: {e}");
                    }
                }
                (payload, name)
            };
            apply_payload(&window, &state, &payload);
            let target_tab = match payload.feature() {
                Feature::Maps => Some(0),
                Feature::Tileable => Some(1),
                Feature::Packing => Some(2),
                Feature::Atlas => Some(3),
                Feature::Optimize => Some(4),
                Feature::Batch => Some(6),
                Feature::Project => None,
            };
            if let Some(t) = target_tab {
                window.global::<Nav>().set_tab(t);
            }
            refresh_list(&window, &store, &locale);
            api.set_status(
                locale.tr_args("presets-status-applied", &[("name", name.as_str().into())]).into(),
            );
        }
    });
    window.global::<PresetsApi>().on_delete_preset({
        let weak = window.as_weak().clone();
        let locale = locale.clone();
        let store = store.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PresetsApi>();
            let Some(i) = sel_idx(api.get_selected()) else {
                api.set_status(locale.tr("presets-status-select-first").into());
                return;
            };
            let name = {
                let mut presets = store.lock().unwrap_or_else(|e| e.into_inner());
                if i >= presets.len() {
                    return;
                }
                let name = presets[i].name.clone();
                presets.remove(i);
                if let Err(e) = save_all(&presets) {
                    api.set_status(
                        locale
                            .tr_args("presets-status-export-error", &[("error", e.as_str().into())])
                            .into(),
                    );
                    return;
                }
                name
            };
            refresh_list(&window, &store, &locale);
            api.set_status(
                locale.tr_args("presets-status-deleted", &[("name", name.as_str().into())]).into(),
            );
        }
    });
    window.global::<PresetsApi>().on_select_preset({
        let weak = window.as_weak().clone();
        move |i| {
            let Some(window) = weak.upgrade() else { return };
            window.global::<PresetsApi>().set_selected(i);
        }
    });
    window.global::<PresetsApi>().on_export_preset({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let store = store.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PresetsApi>();
            if state.gate.is_locked(Capability::PresetsExportImport) {
                api.set_status(locale.tr("presets-locked-export").into());
                return;
            }
            let Some(i) = sel_idx(api.get_selected()) else {
                api.set_status(locale.tr("presets-status-select-first").into());
                return;
            };
            let preset = store.lock().unwrap_or_else(|e| e.into_inner()).get(i).cloned();
            let Some(preset) = preset else { return };
            let text = match export_text(&preset) {
                Ok(t) => t,
                Err(e) => {
                    api.set_status(
                        locale
                            .tr_args("presets-status-export-error", &[("error", e.as_str().into())])
                            .into(),
                    );
                    return;
                }
            };
            let safe: String = sanitize_name(&preset.name)
                .chars()
                .map(|c| match c {
                    ':' | '/' | '\\' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '_',
                    _ => c,
                })
                .collect();
            let file_name =
                format!("{}.texelbox-preset", if safe.is_empty() { "preset".to_string() } else { safe });
            let Some(path) = rfd::FileDialog::new()
                .set_title(locale.tr("presets-export"))
                .add_filter(locale.tr("presets-export"), &["texelbox-preset"])
                .set_file_name(&file_name)
                .save_file()
            else {
                return;
            };
            if let Err(e) = std::fs::write(&path, text) {
                api.set_status(
                    locale
                        .tr_args("presets-status-export-error", &[("error", e.to_string().as_str().into())])
                        .into(),
                );
                return;
            }
            api.set_status(
                locale
                    .tr_args(
                        "presets-status-exported",
                        &[("path", path.to_string_lossy().as_ref().into())],
                    )
                    .into(),
            );
        }
    });
    window.global::<PresetsApi>().on_import_preset({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let store = store.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PresetsApi>();
            if state.gate.is_locked(Capability::PresetsExportImport) {
                api.set_status(locale.tr("presets-locked-export").into());
                return;
            }
            let Some(path) = rfd::FileDialog::new()
                .set_title(locale.tr("presets-import"))
                .add_filter(locale.tr("presets-import"), &["texelbox-preset"])
                .pick_file()
            else {
                return;
            };
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    api.set_status(
                        locale
                            .tr_args("presets-status-import-error", &[("error", e.to_string().as_str().into())])
                            .into(),
                    );
                    return;
                }
            };
            let preset = match import_text(&text) {
                Ok(mut p) => {
                    p.last_used = now();
                    let mut n = 2u32;
                    let base = p.name.clone();
                    let existing_names: Vec<String> = store.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|x| x.name.clone()).collect();
                    while existing_names.contains(&p.name) {
                        p.name = format!("{base} ({n})");
                        n += 1;
                    }
                    if state.gate.is_locked(Capability::PresetsUnlimited) {
                        let feature = p.payload.feature();
                        let count = store
                            .lock()
                            .unwrap()
                            .iter()
                            .filter(|x| x.payload.feature() == feature)
                            .count();
                        if count >= FREE_MAX_PER_FEATURE {
                            api.set_status(
                                locale
                                    .tr_args(
                                        "presets-status-cap",
                                        &[("max", (FREE_MAX_PER_FEATURE as i64).into())],
                                    )
                                    .into(),
                            );
                            return;
                        }
                    }
                    p
                }
                Err(e) => {
                    api.set_status(
                        locale
                            .tr_args("presets-status-import-error", &[("error", e.as_str().into())])
                            .into(),
                    );
                    return;
                }
            };
            let name = preset.name.clone();
            {
                let mut presets = store.lock().unwrap_or_else(|e| e.into_inner());
                presets.push(preset);
                if let Err(e) = save_all(&presets) {
                    presets.pop();
                    api.set_status(
                        locale
                            .tr_args("presets-status-export-error", &[("error", e.as_str().into())])
                            .into(),
                    );
                    return;
                }
            }
            refresh_list(&window, &store, &locale);
            api.set_status(
                locale.tr_args("presets-status-imported", &[("name", name.as_str().into())]).into(),
            );
        }
    });
    {
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let store = store.clone();
        let rx = state.bus.subscribe();
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                if !matches!(event, tbx_state::AppEvent::LanguageChanged) {
                    continue;
                }
                let _ = slint::invoke_from_event_loop({
                    let weak = weak.clone();
                    let locale = locale.clone();
                    let store = store.clone();
                    move || {
                        let Some(window) = weak.upgrade() else { return };
                        refresh_list(&window, &store, &locale);
                    }
                });
            }
        });
    }
}
