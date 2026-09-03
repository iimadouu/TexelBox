use std::sync::{Arc, Mutex};
use image::RgbaImage;
use slint::ComponentHandle;
use tbx_core::io;
use tbx_core::buffer::GrayF32;
use tbx_core::packing::{self, ChannelSource, EnginePreset, MapOutput, ResolvedSource};
use tbx_entitlements::Capability;
use tbx_locale::LocaleManager;
use tbx_state::{AppState, ImageSlot, Stage};
use crate::maps_panel::rgba_to_slint;
use crate::{MainWindow, PackingApi};
const SRC_EMPTY: i32 = 0;
const SRC_CONST_0: i32 = 1;
const SRC_CONST_255: i32 = 2;
const SRC_MAPS_ROUGHNESS: i32 = 3;
const SRC_MAPS_AO: i32 = 4;
const SRC_MAPS_HEIGHT: i32 = 5;
const SRC_CUSTOM: i32 = 6;
const SRC_MAPS_NORMAL: i32 = 7;
enum ChanData {
    Const(u8),
    Gray(GrayF32),
}
fn source_from_index(i: i32) -> ChannelSource {
    match i {
        SRC_CONST_0 => ChannelSource::Constant(0),
        SRC_CONST_255 => ChannelSource::Constant(255),
        SRC_MAPS_ROUGHNESS => ChannelSource::MapOutput(MapOutput::Roughness),
        SRC_MAPS_AO => ChannelSource::MapOutput(MapOutput::Ao),
        SRC_MAPS_HEIGHT => ChannelSource::MapOutput(MapOutput::Height),
        SRC_MAPS_NORMAL => ChannelSource::MapOutput(MapOutput::Normal),
        SRC_CUSTOM => ChannelSource::Custom,
        _ => ChannelSource::Empty,
    }
}
fn index_from_source(s: ChannelSource) -> i32 {
    match s {
        ChannelSource::Empty => SRC_EMPTY,
        ChannelSource::Constant(0) => SRC_CONST_0,
        ChannelSource::Constant(_) => SRC_CONST_255,
        ChannelSource::MapOutput(MapOutput::Roughness) => SRC_MAPS_ROUGHNESS,
        ChannelSource::MapOutput(MapOutput::Ao) => SRC_MAPS_AO,
        ChannelSource::MapOutput(MapOutput::Height) => SRC_MAPS_HEIGHT,
        ChannelSource::MapOutput(MapOutput::Normal) => SRC_MAPS_NORMAL,
        ChannelSource::Custom => SRC_CUSTOM,
    }
}
fn map_output_name(o: MapOutput) -> &'static str {
    match o {
        MapOutput::Roughness => "Roughness",
        MapOutput::Ao => "AO",
        MapOutput::Height => "Height",
        MapOutput::Normal => "Normal",
    }
}
struct PackFailure {
    key: &'static str,
    arg_key: &'static str,
    arg_value: String,
}
fn run_pack(
    state: &AppState,
    custom: &Mutex<[Option<RgbaImage>; 4]>,
    indices: [i32; 4],
) -> Result<RgbaImage, PackFailure> {
    let alpha_locked = state.gate.is_locked(Capability::ChannelPackAlphaChannel);
    let alpha_is_variadic = indices[3] != SRC_EMPTY
        && indices[3] != SRC_CONST_0
        && indices[3] != SRC_CONST_255;
    if alpha_locked && alpha_is_variadic {
        return Err(PackFailure { key: "pack-status-alpha-locked", arg_key: "", arg_value: String::new() });
    }
    let maps_guard = state.project.read().unwrap_or_else(|e| e.into_inner());
    let maps = match maps_guard.maps.outputs.as_ref() {
        Some(m) => m,
        None => {
            return Err(PackFailure {
                key: "pack-status-missing",
                arg_key: "source",
                arg_value: "maps".into(),
            });
        }
    };
    let sources = source_from_index(indices[0]);
    let mapping = [
        sources,
        source_from_index(indices[1]),
        source_from_index(indices[2]),
        source_from_index(indices[3]),
    ];
    let custom_guard = custom.lock().unwrap_or_else(|e| e.into_inner());
    let mut data: Vec<Option<ChanData>> = Vec::with_capacity(4);
    let mut dims: [Option<(u32, u32)>; 4] = [None; 4];
    for (c, src) in mapping.iter().enumerate() {
        let chan = match src {
            ChannelSource::Empty => None,
            ChannelSource::Constant(v) => Some(ChanData::Const(*v)),
            ChannelSource::MapOutput(o) => {
                let gray = match o {
                    MapOutput::Roughness => maps.roughness.clone(),
                    MapOutput::Ao => maps.ao.clone(),
                    MapOutput::Height => maps.height.clone(),
                    MapOutput::Normal => maps.normal.as_ref().map(|n| GrayF32::from_rgba_channel(n, 2)),
                };
                let Some(g) = gray else {
                    return Err(PackFailure {
                        key: "pack-status-missing",
                        arg_key: "source",
                        arg_value: map_output_name(*o).to_string(),
                    });
                };
                dims[c] = Some((g.width, g.height));
                Some(ChanData::Gray(g))
            }
            ChannelSource::Custom => {
                let Some(img) = custom_guard[c].as_ref() else {
                    return Err(PackFailure {
                        key: "pack-status-missing",
                        arg_key: "source",
                        arg_value: format!("Custom ({})", ['R', 'G', 'B', 'A'][c]),
                    });
                };
                dims[c] = Some(img.dimensions());
                Some(ChanData::Gray(GrayF32::from_rgba(img)))
            }
        };
        data.push(chan);
    }
    let (w, h) = packing::check(&mapping, &dims).map_err(|e| match e {
        packing::PackError::ChannelEmpty(ch) => PackFailure {
            key: "pack-status-empty",
            arg_key: "channel",
            arg_value: ch.to_string(),
        },
        packing::PackError::SizeMismatch(_) => PackFailure {
            key: "pack-status-mismatch",
            arg_key: "",
            arg_value: String::new(),
        },
        packing::PackError::AllConstant => PackFailure {
            key: "pack-status-all-constant",
            arg_key: "",
            arg_value: String::new(),
        },
    })?;
    let resolved: [ResolvedSource<'_>; 4] = std::array::from_fn(|c| match data[c].as_ref().unwrap() {
        ChanData::Const(v) => ResolvedSource::Constant(*v),
        ChanData::Gray(g) => ResolvedSource::Gray(g),
    });
    Ok(packing::pack(w, h, &resolved))
}
pub fn sync_entitlements(window: &MainWindow, state: &AppState) {
    let api = window.global::<PackingApi>();
    api.set_lock_presets(state.gate.is_locked(Capability::ChannelPackEnginePresets));
    api.set_lock_alpha(state.gate.is_locked(Capability::ChannelPackAlphaChannel));
}
fn set_failure(window: &MainWindow, locale: &LocaleManager, f: PackFailure) {
    let api = window.global::<PackingApi>();
    let args: Vec<(&str, fluent_bundle::FluentValue)> = if f.arg_key.is_empty() {
        vec![]
    } else {
        vec![(f.arg_key, f.arg_value.as_str().into())]
    };
    api.set_status(locale.tr_args(f.key, &args).into());
}
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let weak = window.as_weak().clone();
    let custom: Arc<Mutex<[Option<RgbaImage>; 4]>> = Arc::new(Mutex::new(Default::default()));
    window.global::<PackingApi>().on_load_slot({
        let weak = weak.clone();
        let locale = locale.clone();
        let custom = custom.clone();
        let state = state.clone();
        move |slot: i32| {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PackingApi>();
            if slot == 3 && state.gate.is_locked(Capability::ChannelPackAlphaChannel) {
                api.set_status(locale.tr("pack-status-alpha-locked").into());
                return;
            }
            let picked = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
                .pick_file();
            let Some(path) = picked else { return };
            match io::load(&path) {
                Ok(img) => {
                    if let Some(c) = custom.lock().unwrap_or_else(|e| e.into_inner()).get_mut(slot as usize) {
                        *c = Some(img);
                    }
                    match slot {
                        0 => api.set_source_r(SRC_CUSTOM),
                        1 => api.set_source_g(SRC_CUSTOM),
                        2 => api.set_source_b(SRC_CUSTOM),
                        3 => api.set_source_a(SRC_CUSTOM),
                        _ => {}
                    }
                    api.set_status(
                        locale
                            .tr_args(
                                "pack-status-loaded",
                                &[("channel", ['R', 'G', 'B', 'A'][slot.clamp(0, 3) as usize].to_string().as_str().into())],
                            )
                            .into(),
                    );
                }
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
    window.global::<PackingApi>().on_apply_preset({
        let weak = weak.clone();
        let locale = locale.clone();
        let custom = custom.clone();
        let state = state.clone();
        move |preset: i32| {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PackingApi>();
            if state.gate.is_locked(Capability::ChannelPackEnginePresets) {
                api.set_status(locale.tr("pack-locked-presets").into());
                return;
            }
            let engine = match preset {
                1 => EnginePreset::UnrealOrm,
                2 => EnginePreset::UnityMetallic,
                3 => EnginePreset::Godot,
                _ => EnginePreset::Custom,
            };
            let mut mapping = packing::preset_mapping(engine);
            if state.gate.is_locked(Capability::ChannelPackAlphaChannel)
                && !matches!(mapping[3], ChannelSource::Constant(_) | ChannelSource::Empty)
            {
                mapping[3] = ChannelSource::Constant(255);
            }
            for slot in mapping.iter_mut() {
                if matches!(slot, ChannelSource::Empty) {
                    *slot = ChannelSource::Constant(0);
                }
            }
            api.set_source_r(index_from_source(mapping[0]));
            api.set_source_g(index_from_source(mapping[1]));
            api.set_source_b(index_from_source(mapping[2]));
            api.set_source_a(index_from_source(mapping[3]));
            let indices = [api.get_source_r(), api.get_source_g(), api.get_source_b(), api.get_source_a()];
            match run_pack(&state, &custom, indices) {
                Ok(packed) => {
                    let (w, h) = packed.dimensions();
                    let papi = window.global::<PackingApi>();
                    papi.set_preview(rgba_to_slint(&packed));
                    papi.set_has_preview(true);
                    papi.set_status(
                        locale
                            .tr_args(
                                "pack-status-ok",
                                &[("w", w.to_string().as_str().into()), ("h", h.to_string().as_str().into())],
                            )
                            .into(),
                    );
                }
                Err(f) => set_failure(&window, &locale, f),
            }
        }
    });
    window.global::<PackingApi>().on_update_preview({
        let weak = weak.clone();
        let locale = locale.clone();
        let custom = custom.clone();
        let state = state.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PackingApi>();
            let indices = [api.get_source_r(), api.get_source_g(), api.get_source_b(), api.get_source_a()];
            match run_pack(&state, &custom, indices) {
                Ok(packed) => {
                    let (w, h) = packed.dimensions();
                    api.set_preview(rgba_to_slint(&packed));
                    api.set_has_preview(true);
                    api.set_status(
                        locale
                            .tr_args(
                                "pack-status-ok",
                                &[("w", w.to_string().as_str().into()), ("h", h.to_string().as_str().into())],
                            )
                            .into(),
                    );
                }
                Err(f) => {
                    api.set_has_preview(false);
                    set_failure(&window, &locale, f);
                }
            }
        }
    });
    window.global::<PackingApi>().on_export_pack({
        let weak = weak.clone();
        let locale = locale.clone();
        let custom = custom.clone();
        let state = state.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PackingApi>();
            let indices = [api.get_source_r(), api.get_source_g(), api.get_source_b(), api.get_source_a()];
            let packed = match run_pack(&state, &custom, indices) {
                Ok(p) => p,
                Err(f) => {
                    set_failure(&window, &locale, f);
                    return;
                }
            };
            let picked = rfd::FileDialog::new()
                .set_file_name("packed_orm.png")
                .add_filter("PNG", &["png"])
                .add_filter("TGA", &["tga"])
                .save_file();
            let Some(path) = picked else { return };
            match io::save(&path, &packed) {
                Ok(()) => {
                    state.update(Stage::ChannelPack, |p| {
                        p.packed = Some(ImageSlot {
                            name: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "packed".into()),
                            origin: path.display().to_string(),
                            image: Arc::new(packed),
                        });
                    });
                    api.set_status(
                        locale
                            .tr_args("maps-status-saved", &[("path", path.display().to_string().as_str().into())])
                            .into(),
                    );
                }
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
    window.global::<PackingApi>().on_quick_export_pack({
        let weak = weak.clone();
        let locale = locale.clone();
        let custom = custom.clone();
        let state = state.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<PackingApi>();
            let indices = [api.get_source_r(), api.get_source_g(), api.get_source_b(), api.get_source_a()];
            let packed = match run_pack(&state, &custom, indices) {
                Ok(p) => p,
                Err(f) => {
                    set_failure(&window, &locale, f);
                    return;
                }
            };
            let picked = rfd::FileDialog::new()
                .set_file_name("packed_orm.png")
                .add_filter("PNG", &["png"])
                .add_filter("TGA", &["tga"])
                .save_file();
            let Some(path) = picked else { return };
            match io::save(&path, &packed) {
                Ok(()) => {
                    state.update(Stage::ChannelPack, |p| {
                        p.packed = Some(ImageSlot {
                            name: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "packed".into()),
                            origin: path.display().to_string(),
                            image: Arc::new(packed),
                        });
                    });
                    api.set_status(
                        locale
                            .tr_args("maps-status-saved", &[("path", path.display().to_string().as_str().into())])
                            .into(),
                    );
                }
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
}
pub fn apply_maps_handoff(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let (has_ao, has_rough, has_height, _has_normal) = {
        let p = state.project.read().unwrap_or_else(|e| e.into_inner());
        let outs = p.maps.outputs.as_ref();
        (
            outs.and_then(|o| o.ao.as_ref()).is_some(),
            outs.and_then(|o| o.roughness.as_ref()).is_some(),
            outs.and_then(|o| o.height.as_ref()).is_some(),
            outs.and_then(|o| o.normal.as_ref()).is_some(),
        )
    };
    let api = window.global::<PackingApi>();
    api.set_source_r(if has_ao { SRC_MAPS_AO } else { SRC_CONST_0 });
    api.set_source_g(if has_rough { SRC_MAPS_ROUGHNESS } else { SRC_CONST_0 });
    api.set_source_b(if has_height { SRC_MAPS_HEIGHT } else { SRC_CONST_0 });
    api.set_source_a(SRC_CONST_255);
    window.global::<crate::Nav>().set_tab(2);
    api.set_status(locale.tr("pack-status-handoff").into());
    let indices = [api.get_source_r(), api.get_source_g(), api.get_source_b(), api.get_source_a()];
    let state_w = state.clone();
    let locale_w = locale.clone();
    let weak_w = window.as_weak();
    std::thread::spawn(move || {
        let custom: Mutex<[Option<RgbaImage>; 4]> = Mutex::new(Default::default());
        let result = run_pack(&state_w, &custom, indices);
        let _ = slint::invoke_from_event_loop(move || {
            let Some(window) = weak_w.upgrade() else { return };
            let api = window.global::<PackingApi>();
            match result {
                Ok(packed) => {
                    api.set_preview(rgba_to_slint(&packed));
                    api.set_has_preview(true);
                    let (w, h) = packed.dimensions();
                    api.set_status(
                        locale_w
                            .tr_args(
                                "pack-status-ok",
                                &[("w", w.to_string().as_str().into()), ("h", h.to_string().as_str().into())],
                            )
                            .into(),
                    );
                }
                Err(f) => set_failure(&window, &locale_w, f),
            }
        });
    });
}
