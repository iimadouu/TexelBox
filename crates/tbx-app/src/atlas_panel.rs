use std::sync::{Arc, Mutex};
use std::time::Instant;
use image::RgbaImage;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tbx_core::atlas::{self, AtlasParams, AtlasResult, SidecarFormat, FREE_MAX_IMAGES, FREE_MAX_SIZE};
use tbx_core::io;
use tbx_entitlements::Capability;
use tbx_locale::LocaleManager;
use tbx_state::{AppState, ImageSlot, Stage};
use crate::maps_panel::{rgba_to_slint, FREE_MAX_SIDE};
use crate::{AtlasApi, MainWindow, MapsApi, Nav, PackingApi};
type ImageList = Arc<Mutex<Vec<(String, Arc<RgbaImage>, u8)>>>;
const SIZE_PRESETS: [u32; 5] = [512, 1024, 2048, 4096, 8192];
struct Built {
    result: AtlasResult,
    sidecar: SidecarFormat,
}
pub fn sync_entitlements(window: &MainWindow, state: &AppState) {
    let api = window.global::<AtlasApi>();
    api.set_lock_unlimited(state.gate.is_locked(Capability::AtlasUnlimitedImages));
    api.set_lock_size_hi(state.gate.is_locked(Capability::AtlasSize8192));
    api.set_lock_pad_bleed(state.gate.is_locked(Capability::AtlasBleedPaddingControl));
    api.set_lock_rotation(state.gate.is_locked(Capability::AtlasRotationPacking));
    api.set_lock_sidecar_extra(state.gate.is_locked(Capability::AtlasSidecarFormatsExtra));
    api.set_lock_trim(state.gate.is_locked(Capability::AtlasTrimSheetMode));
    api.set_lock_priority_arrange(state.gate.is_locked(Capability::AtlasPriorityArrange));
}
fn refresh_list(window: &MainWindow, images: &[(String, Arc<RgbaImage>, u8)]) {
    let api = window.global::<AtlasApi>();
    let names: Vec<SharedString> = images
        .iter()
        .map(|(name, img, _pri)| {
            let (w, h) = img.dimensions();
            format!("{name} — {w}×{h}").into()
        })
        .collect();
    let priorities: Vec<i32> = images.iter().map(|(_, _, pri)| *pri as i32).collect();
    api.set_image_names(ModelRc::new(VecModel::from(names)));
    api.set_image_priorities(ModelRc::new(VecModel::from(priorities)));
    api.set_image_count(images.len() as i32);
    api.set_selected_index(-1);
    api.set_selected_priority(3);
    api.set_priority_idx(2);
}
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let weak = window.as_weak().clone();
    let images: ImageList = Arc::new(Mutex::new(Vec::new()));
    let built: Arc<Mutex<Option<Built>>> = Arc::new(Mutex::new(None));
    let prev_selected: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
    window.global::<AtlasApi>().on_add_images({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        let images = images.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            let picked = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
                .pick_files();
            let Some(paths) = picked else { return };
            let cap = if state.gate.is_locked(Capability::AtlasUnlimitedImages) {
                Some(FREE_MAX_IMAGES)
            } else {
                None
            };
            let mut added = 0usize;
            let mut capped = false;
            {
                let mut list = images.lock().unwrap_or_else(|e| e.into_inner());
                for path in paths {
                    if cap.is_some_and(|c| list.len() >= c) {
                        capped = true;
                        break;
                    }
                    match io::load(&path) {
                        Ok(img) => {
                            let name = path
                                .file_stem()
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "image".into());
                            list.push((name, Arc::new(img), 3));
                            added += 1;
                        }
                        Err(e) => {
                            api.set_status(format!("{}: {}", locale.tr("common-error"), e).into());
                        }
                    }
                }
                refresh_list(&window, &list);
            }
            if capped {
                api.set_status(
                    locale
                        .tr_args("atlas-status-cap", &[("max", tbx_locale::num(FREE_MAX_IMAGES as f64))])
                        .into(),
                );
            } else if added > 0 {
                api.set_status(
                    locale
                        .tr_args("atlas-status-loaded", &[("count", tbx_locale::num(added as f64))])
                        .into(),
                );
            }
        }
    });
    window.global::<AtlasApi>().on_add_packed({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        let images = images.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            let packed = state.project.read().unwrap_or_else(|e| e.into_inner()).packed.clone();
            let Some(slot) = packed else {
                api.set_status(locale.tr("atlas-status-no-packed").into());
                return;
            };
            let mut list = images.lock().unwrap_or_else(|e| e.into_inner());
            if state.gate.is_locked(Capability::AtlasUnlimitedImages) && list.len() >= FREE_MAX_IMAGES {
                api.set_status(
                    locale
                        .tr_args("atlas-status-cap", &[("max", tbx_locale::num(FREE_MAX_IMAGES as f64))])
                        .into(),
                );
                return;
            }
            list.push((slot.name, slot.image, 3));
            refresh_list(&window, &list);
            api.set_status(locale.tr("atlas-status-added-packed").into());
        }
    });
    window.global::<AtlasApi>().on_remove_selected({
        let weak = weak.clone();
        let locale = locale.clone();
        let images = images.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            let idx = api.get_selected_index();
            if idx < 0 {
                api.set_status(locale.tr("atlas-status-none-selected").into());
                return;
            }
            let mut list = images.lock().unwrap_or_else(|e| e.into_inner());
            if (idx as usize) < list.len() {
                list.remove(idx as usize);
                refresh_list(&window, &list);
                api.set_status(locale.tr("atlas-status-removed").into());
            }
        }
    });
    window.global::<AtlasApi>().on_selection_changed({
        let weak = weak.clone();
        let images = images.clone();
        let prev_selected = prev_selected.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            let new_idx = api.get_selected_index();
            let mut list = images.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(old_idx) = *prev_selected.lock().unwrap_or_else(|e| e.into_inner()) {
                if old_idx < list.len() {
                    list[old_idx].2 = (api.get_priority_idx() + 1).clamp(1, 5) as u8;
                }
            }
            if new_idx >= 0 && (new_idx as usize) < list.len() {
                let pri = list[new_idx as usize].2 as i32;
                api.set_selected_priority(pri);
                api.set_priority_idx((pri - 1).clamp(0, 4) as i32);
            } else {
                api.set_selected_priority(3);
                api.set_priority_idx(2);
            }
            *prev_selected.lock().unwrap_or_else(|e| e.into_inner()) = if new_idx >= 0 { Some(new_idx as usize) } else { None };
        }
    });
    window.global::<AtlasApi>().on_set_priority({
        let weak = weak.clone();
        let locale = locale.clone();
        let images = images.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            let idx = api.get_selected_index();
            if idx < 0 {
                api.set_status(locale.tr("atlas-status-none-selected").into());
                return;
            }
            let mut list = images.lock().unwrap_or_else(|e| e.into_inner());
            if (idx as usize) < list.len() {
                list[idx as usize].2 = (api.get_priority_idx() + 1).clamp(1, 5) as u8;
                refresh_list(&window, &list);
                api.set_status(
                    locale
                        .tr_args("atlas-status-priority-set", &[("priority", tbx_locale::num(list[idx as usize].2 as f64))])
                        .into(),
                );
            }
        }
    });
    window.global::<AtlasApi>().on_priority_arrange({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        let images = images.clone();
        let built = built.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            if api.get_busy() { return; }
            if state.gate.is_locked(Capability::AtlasPriorityArrange) {
                api.set_status(locale.tr("atlas-status-priority-locked").into());
                return;
            }
            let mut list = images.lock().unwrap_or_else(|e| e.into_inner());
            if list.is_empty() {
                api.set_status(locale.tr("atlas-status-empty").into());
                return;
            }
            let idx = api.get_selected_index();
            if idx >= 0 && (idx as usize) < list.len() {
                list[idx as usize].2 = (api.get_priority_idx() + 1).clamp(1, 5) as u8;
            }
            list.sort_by(|a, b| b.2.cmp(&a.2));
            refresh_list(&window, &list);
            api.set_busy(true);
            api.set_status(locale.tr("common-busy").into());
            let inputs: Vec<(String, Arc<RgbaImage>, u8)> = list.clone();
            let locale_w = locale.clone();
            let state_w = state.clone();
            let built_w = built.clone();
            let weak_w = weak.clone();
            std::thread::spawn(move || {
                let inputs: Vec<atlas::AtlasInput<'_>> = inputs
                    .iter()
                    .map(|(name, img, pri)| atlas::AtlasInput { name: name.as_str(), image: img.as_ref(), priority: *pri })
                    .collect();
                let mut params = AtlasParams::default();
                params.priority_arrange = true;
                let outcome = atlas::pack(&inputs, &params);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak_w.upgrade() else { return };
                    let api = window.global::<AtlasApi>();
                    api.set_busy(false);
                    let result = match outcome {
                        Ok(r) => r,
                        Err(e) => {
                            api.set_has_atlas(false);
                            let msg = match e {
                                atlas::AtlasError::DoesNotFit(max) => locale_w
                                    .tr_args("atlas-status-no-fit", &[("max", tbx_locale::num(max as f64))]),
                                other => format!("{}: {}", locale_w.tr("common-error"), other),
                            };
                            api.set_status(msg.into());
                            return;
                        }
                    };
                    let n_tiles = result.tiles.len();
                    let size = result.size;
                    let preview_img = if result.image.dimensions().0 > 512 || result.image.dimensions().1 > 512 {
                        tbx_core::io::downscale_to_fit(&result.image, 512)
                    } else {
                        result.image.clone()
                    };
                    let preview = rgba_to_slint(&preview_img);
                    let image_arc = Arc::new(result.image.clone());
                    api.set_status(
                        locale_w
                            .tr_args(
                                "atlas-status-priority-built",
                                &[
                                    ("tiles", n_tiles.to_string().as_str().into()),
                                    ("size", size.to_string().as_str().into()),
                                ],
                            )
                            .into(),
                    );
                    api.set_atlas_info(
                        locale_w
                            .tr_args(
                                "atlas-info",
                                &[
                                    ("size", size.to_string().as_str().into()),
                                    ("tiles", n_tiles.to_string().as_str().into()),
                                    ("format", "JSON".into()),
                                ],
                            )
                            .into(),
                    );
                    api.set_atlas_preview(preview);
                    api.set_has_atlas(true);
                    state_w.update(Stage::Atlas, |p| {
                        p.atlas = Some(ImageSlot {
                            name: format!("atlas_{size}"),
                            origin: format!("generated:atlas ({n_tiles} tiles)"),
                            image: image_arc,
                        });
                    });
                    *built_w.lock().unwrap_or_else(|e| e.into_inner()) = Some(Built { result, sidecar: SidecarFormat::Json });
                });
            });
        }
    });
    window.global::<AtlasApi>().on_send_to_maps({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            let atlas_slot = {
                let p = state.project.read().unwrap_or_else(|e| e.into_inner());
                p.atlas.clone()
            };
            let Some(slot) = atlas_slot else {
                api.set_status(locale.tr("atlas-status-no-atlas").into());
                return;
            };
            let (w, h) = slot.image.dimensions();
            state.update(Stage::Source, |p| {
                p.source = Some(ImageSlot {
                    name: slot.name.clone(),
                    origin: format!("atlas:{}x{}", w, h),
                    image: slot.image.clone(),
                });
                p.maps.outputs = None;
            });
            let maps_api = window.global::<MapsApi>();
            let preview = if w > FREE_MAX_SIDE || h > FREE_MAX_SIDE {
                tbx_core::io::downscale_to_fit(&slot.image, FREE_MAX_SIDE)
            } else {
                (*slot.image).clone()
            };
            maps_api.set_source_image(rgba_to_slint(&preview));
            maps_api.set_source_name(slot.name.into());
            maps_api.set_has_source(true);
            maps_api.set_resolution_label(format!("{w}×{h} px").into());
            maps_api.set_has_normal(false);
            maps_api.set_has_height(false);
            maps_api.set_has_roughness(false);
            maps_api.set_has_ao(false);
            window.global::<PackingApi>().set_has_preview(false);
            window.global::<Nav>().set_tab(0);
            api.set_status(locale.tr("atlas-status-sent-maps").into());
        }
    });
    window.global::<AtlasApi>().on_build({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        let images = images.clone();
        let built = built.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            if api.get_busy() {
                return;
            }
            let list: Vec<(String, Arc<RgbaImage>, u8)> = images.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if list.is_empty() {
                api.set_status(locale.tr("atlas-status-empty").into());
                return;
            }
            api.set_busy(true);
            api.set_status(locale.tr("common-busy").into());
            let mut notes: Vec<String> = Vec::new();
            let mut list = list;
            if state.gate.is_locked(Capability::AtlasUnlimitedImages) && list.len() > FREE_MAX_IMAGES {
                list.truncate(FREE_MAX_IMAGES);
                notes.push(
                    locale.tr_args("atlas-status-cap", &[("max", tbx_locale::num(FREE_MAX_IMAGES as f64))]),
                );
            }
            let mut max_size = SIZE_PRESETS[api.get_size_index().clamp(0, 4) as usize];
            if state.gate.is_locked(Capability::AtlasSize8192) && max_size > FREE_MAX_SIZE {
                max_size = FREE_MAX_SIZE;
                notes.push(
                    locale.tr_args("atlas-status-size-capped", &[("max", tbx_locale::num(FREE_MAX_SIZE as f64))]),
                );
            }
            let pad_bleed_locked = state.gate.is_locked(Capability::AtlasBleedPaddingControl);
            let padding = if pad_bleed_locked { 2 } else { (api.get_padding().round() as u32).min(16) };
            let bleed = if pad_bleed_locked { 0 } else { (api.get_bleed().round() as u32).min(8) };
            let allow_rotation =
                !state.gate.is_locked(Capability::AtlasRotationPacking) && api.get_allow_rotation();
            let trim_sheet = !state.gate.is_locked(Capability::AtlasTrimSheetMode) && api.get_trim_sheet();
            let sidecar = if state.gate.is_locked(Capability::AtlasSidecarFormatsExtra) || api.get_sidecar_index() != 1 {
                SidecarFormat::Json
            } else {
                SidecarFormat::Xml
            };
            let params = AtlasParams { max_size, padding, bleed, allow_rotation, sidecar, trim_sheet, priority_arrange: false };
            let weak_w = weak.clone();
            let locale_w = locale.clone();
            let state_w = state.clone();
            let built_w = built.clone();
            std::thread::spawn(move || {
                let inputs: Vec<atlas::AtlasInput<'_>> = list
                    .iter()
                    .map(|(name, img, pri)| atlas::AtlasInput { name: name.as_str(), image: img.as_ref(), priority: *pri })
                    .collect();
                let t0 = Instant::now();
                let outcome = atlas::pack(&inputs, &params);
                let secs = t0.elapsed().as_secs_f32();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak_w.upgrade() else { return };
                    let api = window.global::<AtlasApi>();
                    api.set_busy(false);
                    let result = match outcome {
                        Ok(r) => r,
                        Err(e) => {
                            api.set_has_atlas(false);
                            let msg = match e {
                                atlas::AtlasError::DoesNotFit(max) => locale_w
                                    .tr_args("atlas-status-no-fit", &[("max", tbx_locale::num(max as f64))]),
                                other => format!("{}: {}", locale_w.tr("common-error"), other),
                            };
                            api.set_status(msg.into());
                            return;
                        }
                    };
                    let n_tiles = result.tiles.len();
                    let size = result.size;
                    let preview_img = if result.image.dimensions().0 > 512 || result.image.dimensions().1 > 512 {
                        tbx_core::io::downscale_to_fit(&result.image, 512)
                    } else {
                        result.image.clone()
                    };
                    let preview = rgba_to_slint(&preview_img);
                    let image_arc = Arc::new(result.image.clone());
                    let fmt_name = match sidecar {
                        SidecarFormat::Json => "JSON",
                        SidecarFormat::Xml => "XML",
                    };
                    let mut status: String = locale_w
                        .tr_args(
                            "atlas-status-built",
                            &[
                                ("tiles", n_tiles.to_string().as_str().into()),
                                ("size", size.to_string().as_str().into()),
                                ("secs", format!("{secs:.2}").as_str().into()),
                            ],
                        );
                    for n in &notes {
                        status = format!("{status} — {n}");
                    }
                    api.set_status(status.into());
                    api.set_atlas_info(
                        locale_w
                            .tr_args(
                                "atlas-info",
                                &[
                                    ("size", size.to_string().as_str().into()),
                                    ("tiles", n_tiles.to_string().as_str().into()),
                                    ("format", fmt_name.into()),
                                ],
                            )
                            .into(),
                    );
                    api.set_atlas_preview(preview);
                    api.set_has_atlas(true);
                    state_w.update(Stage::Atlas, |p| {
                        p.atlas = Some(ImageSlot {
                            name: format!("atlas_{size}"),
                            origin: format!("generated:atlas ({n_tiles} tiles)"),
                            image: image_arc,
                        });
                    });
                    *built_w.lock().unwrap_or_else(|e| e.into_inner()) = Some(Built { result, sidecar });
                });
            });
        }
    });
    window.global::<AtlasApi>().on_export_image({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        let built = built.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            let guard = built.lock().unwrap_or_else(|e| e.into_inner());
            let Some(b) = guard.as_ref() else { return };
            let formats_locked = state.gate.is_locked(Capability::AtlasImageFormats);
            let mut dlg = rfd::FileDialog::new().set_file_name("atlas.png").add_filter("PNG", &["png"]);
            if !formats_locked {
                dlg = dlg.add_filter("TGA", &["tga"]);
            }
            let Some(path) = dlg.save_file() else { return };
            if formats_locked && io::ImageFormat::from_path(&path) != Some(io::ImageFormat::Png) {
                api.set_status(locale.tr("atlas-status-format-locked").into());
                return;
            }
            match io::save(&path, &b.result.image) {
                Ok(()) => api.set_status(
                    locale
                        .tr_args("maps-status-saved", &[("path", path.display().to_string().as_str().into())])
                        .into(),
                ),
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
    window.global::<AtlasApi>().on_export_sidecar({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        let built = built.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            let guard = built.lock().unwrap_or_else(|e| e.into_inner());
            let Some(b) = guard.as_ref() else { return };
            let fmt = if state.gate.is_locked(Capability::AtlasSidecarFormatsExtra) {
                SidecarFormat::Json
            } else {
                b.sidecar
            };
            let (ext, label) = match fmt {
                SidecarFormat::Json => ("json", "JSON"),
                SidecarFormat::Xml => ("xml", "XML"),
            };
            let Some(path) = rfd::FileDialog::new()
                .set_file_name(format!("atlas_sidecar.{ext}"))
                .add_filter(label, &[ext])
                .save_file()
            else {
                return;
            };
            let content = match fmt {
                SidecarFormat::Json => atlas::sidecar_json(&b.result),
                SidecarFormat::Xml => atlas::sidecar_xml(&b.result),
            };
            match std::fs::write(&path, content) {
                Ok(()) => api.set_status(
                    locale
                        .tr_args("maps-status-saved", &[("path", path.display().to_string().as_str().into())])
                        .into(),
                ),
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
    window.global::<AtlasApi>().on_quick_export_atlas({
        let weak = weak.clone();
        let locale = locale.clone();
        let built = built.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<AtlasApi>();
            let guard = built.lock().unwrap_or_else(|e| e.into_inner());
            let Some(b) = guard.as_ref() else {
                api.set_status(locale.tr("atlas-status-no-atlas").into());
                return;
            };
            let Some(path) = rfd::FileDialog::new()
                .set_file_name("atlas.png")
                .add_filter("PNG", &["png"])
                .save_file()
            else {
                return;
            };
            match io::save(&path, &b.result.image) {
                Ok(()) => api.set_status(
                    locale
                        .tr_args("maps-status-saved", &[("path", path.display().to_string().as_str().into())])
                        .into(),
                ),
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
}
