use std::sync::{Arc, Mutex};
use image::RgbaImage;
use slint::{ComponentHandle, Weak};
use tbx_core::{io, tileable};
use tbx_entitlements::Capability;
use tbx_locale::LocaleManager;
use tbx_state::{AppState, ImageSlot, Stage};
use crate::maps_panel::rgba_to_slint;
use crate::{MainWindow, MapsApi, Nav, PackingApi, TileableApi};
const FREE_MAX_SIDE: u32 = 2048;
const PREVIEW_MAX_SIDE: u32 = 512;
const UNDO_LIMIT: usize = 10;
struct Work {
    base: Option<Arc<RgbaImage>>,
    working: Option<Arc<RgbaImage>>,
    name: String,
    undo_stack: Vec<Arc<RgbaImage>>,
    redo_stack: Vec<Arc<RgbaImage>>,
}
type WorkHandle = Arc<Mutex<Work>>;
pub fn sync_entitlements(window: &MainWindow, state: &AppState) {
    let api = window.global::<TileableApi>();
    api.set_lock_auto_heal(state.gate.is_locked(Capability::TileableAutoHeal));
    api.set_lock_preview(state.gate.is_locked(Capability::TileableLiveRepeatPreview));
}
fn refresh_ui(weak: &Weak<MainWindow>, work: &WorkHandle, state: &AppState, _locale: &LocaleManager) {
    let Some(window) = weak.upgrade() else { return };
    let api = window.global::<TileableApi>();
    let (img, name) = {
        let w = work.lock().unwrap_or_else(|e| e.into_inner());
        match &w.working {
            Some(i) => (i.clone(), w.name.clone()),
            None => return,
        }
    };
    api.set_work_image(rgba_to_slint(&img));
    api.set_source_name(name.into());
    api.set_has_work(true);
    if state.gate.is_locked(Capability::TileableLiveRepeatPreview) {
        api.set_has_preview(false);
    } else {
        let pre_image = Arc::clone(&img);
        let weak_w = weak.clone();
        let work_w = work.clone();
        std::thread::spawn(move || {
            let small = io::downscale_to_fit(&pre_image, PREVIEW_MAX_SIDE);
            let tiled = tileable::tile_preview(&small, 3);
            let (tw, th) = tiled.dimensions();
            let raw = tiled.into_raw();
            let _ = slint::invoke_from_event_loop(move || {
                let discarded = {
                    let w = work_w.lock().unwrap_or_else(|e| e.into_inner());
                    !matches!(&w.working, Some(now_img) if Arc::ptr_eq(now_img, &pre_image))
                };
                if let Some(window) = weak_w.upgrade() {
                    let t_api = window.global::<TileableApi>();
                    if !discarded {
                        let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&raw, tw, th);
                        t_api.set_preview_3x3(slint::Image::from_rgba8(buffer));
                        t_api.set_has_preview(true);
                    }
                }
            });
        });
    }
}
fn push_undo(work: &WorkHandle, weak: &Weak<MainWindow>) {
    let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(img) = w.working.clone() {
        if w.undo_stack.len() >= UNDO_LIMIT {
            w.undo_stack.remove(0);
        }
        w.undo_stack.push(img);
        w.redo_stack.clear();
    }
    let can_undo = !w.undo_stack.is_empty();
    let can_redo = !w.redo_stack.is_empty();
    drop(w);
    if let Some(window) = weak.upgrade() {
        let api = window.global::<TileableApi>();
        api.set_can_undo(can_undo);
        api.set_can_redo(can_redo);
    }
}
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let weak = window.as_weak().clone();
    let work: WorkHandle = Arc::new(Mutex::new(Work {
        base: None,
        working: None,
        name: "source".into(),
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    }));
    window.global::<TileableApi>().on_open_source({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<TileableApi>();
            let picked = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
                .pick_file();
            let Some(path) = picked else { return };
            match io::load(&path) {
                Ok(mut img) => {
                    let capped = if state.gate.is_locked(Capability::TileableUnlimitedResolution)
                        && img.dimensions().0.max(img.dimensions().1) > FREE_MAX_SIDE
                    {
                        img = io::downscale_to_fit(&img, FREE_MAX_SIDE);
                        true
                    } else {
                        false
                    };
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "source".into());
                    {
                        let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
                        w.base = Some(Arc::new(img.clone()));
                        w.working = Some(Arc::new(img));
                        w.name = name;
                        w.undo_stack.clear();
                        w.redo_stack.clear();
                    }
                    refresh_ui(&weak, &work, &state, &locale);
                    if let Some(window) = weak.upgrade() {
                        let api = window.global::<TileableApi>();
                        api.set_can_undo(false);
                        api.set_can_redo(false);
                    }
                    if capped {
                        api.set_status(
                            locale
                                .tr_args("maps-resolution-capped", &[("max", tbx_locale::num(FREE_MAX_SIDE as f64))])
                                .into(),
                        );
                    }
                }
                Err(e) => {
                    api.set_status(format!("{}: {}", locale.tr("common-error"), e).into());
                }
            }
        }
    });
    window.global::<TileableApi>().on_apply_offset({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let cur = work.lock().unwrap_or_else(|e| e.into_inner()).working.clone();
            let Some(cur) = cur else { return };
            push_undo(&work, &weak);
            let off = tileable::offset_wrap(&cur);
            work.lock().unwrap_or_else(|e| e.into_inner()).working = Some(Arc::new(off));
            refresh_ui(&weak, &work, &state, &locale);
            window.global::<TileableApi>().set_status(locale.tr("tileable-status-offset").into());
        }
    });
    window.global::<TileableApi>().on_auto_heal({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<TileableApi>();
            if state.gate.is_locked(Capability::TileableAutoHeal) {
                return;
            }
            if api.get_busy() {
                return;
            }
            let cur = work.lock().unwrap_or_else(|e| e.into_inner()).working.clone();
            let Some(cur) = cur else { return };
            let strength = api.get_heal_strength();
            let radius = api.get_heal_radius();
            let passes = api.get_heal_passes().round() as u8;
            api.set_busy(true);
            api.set_status(locale.tr("common-busy").into());
            let pre_heal = Arc::clone(&cur);
            let weak_w = weak.clone();
            let work_w = work.clone();
            let state_w = state.clone();
            let locale_w = locale.clone();
            std::thread::spawn(move || {
                let healed = tileable::auto_heal(&cur, strength, radius, passes.max(1));
                let _ = slint::invoke_from_event_loop(move || {
                    let discarded = {
                        let mut w = work_w.lock().unwrap_or_else(|e| e.into_inner());
                        match &w.working {
                            Some(now_img) if Arc::ptr_eq(now_img, &pre_heal) => {
                                w.working = Some(Arc::new(healed));
                                false
                            }
                            _ => true,
                        }
                    };
                    if let Some(w) = weak_w.upgrade() {
                        let t_api = w.global::<TileableApi>();
                        t_api.set_busy(false);
                        if discarded {
                            t_api.set_status(locale_w.tr("tileable-status-heal-stale").into());
                        }
                    }
                    refresh_ui(&weak_w, &work_w, &state_w, &locale_w);
                });
            });
        }
    });
    window.global::<TileableApi>().on_mirror({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let cur = work.lock().unwrap_or_else(|e| e.into_inner()).working.clone();
            let Some(cur) = cur else { return };
            push_undo(&work, &weak);
            let mirrored = tileable::mirror_tile(&cur);
            work.lock().unwrap_or_else(|e| e.into_inner()).working = Some(Arc::new(mirrored));
            refresh_ui(&weak, &work, &state, &locale);
        }
    });
    window.global::<TileableApi>().on_reset({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let base = work.lock().unwrap_or_else(|e| e.into_inner()).base.clone();
            let Some(base) = base else { return };
            push_undo(&work, &weak);
            work.lock().unwrap_or_else(|e| e.into_inner()).working = Some(base);
            refresh_ui(&weak, &work, &state, &locale);
        }
    });
    window.global::<TileableApi>().on_undo({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let prev = {
                let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(prev_img) = w.undo_stack.pop() {
                    if let Some(cur) = w.working.clone() {
                        if w.redo_stack.len() >= UNDO_LIMIT {
                            w.redo_stack.remove(0);
                        }
                        w.redo_stack.push(cur);
                    }
                    w.working = Some(prev_img.clone());
                    let can_undo = !w.undo_stack.is_empty();
                    let can_redo = !w.redo_stack.is_empty();
                    Some((can_undo, can_redo))
                } else {
                    None
                }
            };
            if let Some((can_undo, can_redo)) = prev {
                if let Some(window) = weak.upgrade() {
                    let api = window.global::<TileableApi>();
                    api.set_can_undo(can_undo);
                    api.set_can_redo(can_redo);
                }
                refresh_ui(&weak, &work, &state, &locale);
            }
        }
    });
    window.global::<TileableApi>().on_redo({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let next = {
                let mut w = work.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(next_img) = w.redo_stack.pop() {
                    if let Some(cur) = w.working.clone() {
                        if w.undo_stack.len() >= UNDO_LIMIT {
                            w.undo_stack.remove(0);
                        }
                        w.undo_stack.push(cur);
                    }
                    w.working = Some(next_img.clone());
                    let can_undo = !w.undo_stack.is_empty();
                    let can_redo = !w.redo_stack.is_empty();
                    Some((can_undo, can_redo))
                } else {
                    None
                }
            };
            if let Some((can_undo, can_redo)) = next {
                if let Some(window) = weak.upgrade() {
                    let api = window.global::<TileableApi>();
                    api.set_can_undo(can_undo);
                    api.set_can_redo(can_redo);
                }
                refresh_ui(&weak, &work, &state, &locale);
            }
        }
    });
    window.global::<TileableApi>().on_brush_at({
        let weak = weak.clone();
        let work = work.clone();
        move |nx: f32, ny: f32| {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<TileableApi>();
            if !api.get_brush_enabled() {
                return;
            }
            let ox = api.get_brush_offset_x().round() as i64;
            let oy = api.get_brush_offset_y().round() as i64;
            let size = api.get_brush_size();
            let cur = work.lock().unwrap_or_else(|e| e.into_inner()).working.clone();
            let Some(cur) = cur else { return };
            push_undo(&work, &weak);
            let mut img = Arc::unwrap_or_clone(cur);
            let px = (nx.clamp(0.0, 1.0) * (img.width() - 1) as f32) as i64;
            let py = (ny.clamp(0.0, 1.0) * (img.height() - 1) as f32) as i64;
            tileable::brush_clone(&mut img, (px, py), (ox, oy), size, 0.6);
            work.lock().unwrap_or_else(|e| e.into_inner()).working = Some(Arc::new(img));
            if let Some(window) = weak.upgrade() {
                let api = window.global::<TileableApi>();
                let img = work.lock().unwrap_or_else(|e| e.into_inner()).working.clone().unwrap();
                api.set_work_image(rgba_to_slint(&img));
            }
        }
    });
    window.global::<TileableApi>().on_save({
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let (img, default_name) = {
                let w = work.lock().unwrap_or_else(|e| e.into_inner());
                let Some(img) = w.working.clone() else { return };
                (img, format!("{}_tileable.png", w.name))
            };
            let api = window.global::<TileableApi>();
            let picked = rfd::FileDialog::new()
                .set_file_name(default_name)
                .add_filter("PNG", &["png"])
                .add_filter("TGA", &["tga"])
                .save_file();
            let Some(path) = picked else { return };
            match io::save(&path, &img) {
                Ok(()) => api.set_status(
                    locale
                        .tr_args("maps-status-saved", &[("path", path.display().to_string().as_str().into())])
                        .into(),
                ),
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
    window.global::<TileableApi>().on_quick_export({
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<TileableApi>();
            let (img, default_name) = {
                let w = work.lock().unwrap_or_else(|e| e.into_inner());
                let Some(img) = w.working.clone() else { return };
                (img, format!("{}_tileable.png", w.name))
            };
            let dir = rfd::FileDialog::new()
                .set_file_name(&default_name)
                .add_filter("PNG", &["png"])
                .add_filter("TGA", &["tga"])
                .save_file();
            let Some(path) = dir else { return };
            match io::save(&path, &img) {
                Ok(()) => api.set_status(
                    locale
                        .tr_args("maps-status-saved", &[("path", path.display().to_string().as_str().into())])
                        .into(),
                ),
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
    window.global::<TileableApi>().on_send_to_maps({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        let work = work.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let (img, name) = {
                let w = work.lock().unwrap_or_else(|e| e.into_inner());
                let Some(img) = w.working.clone() else { return };
                (img, w.name.clone())
            };
            let slot_name = format!("{name}_tileable");
            state.update(Stage::Tileable, |p| {
                p.tileable = Some(ImageSlot {
                    image: img.clone(),
                    name: slot_name.clone(),
                    origin: "generated:tileable".into(),
                });
            });
            state.update(Stage::Source, |p| {
                p.source = Some(ImageSlot {
                    image: img.clone(),
                    name: slot_name.clone(),
                    origin: "generated:tileable".into(),
                });
                p.maps.outputs = None;
            });
            let maps = window.global::<MapsApi>();
            let (w, h) = img.dimensions();
            maps.set_source_image(rgba_to_slint(&img));
            maps.set_source_name(slot_name.into());
            maps.set_has_source(true);
            maps.set_resolution_label(format!("{w}×{h} px").into());
            maps.set_has_normal(false);
            maps.set_has_height(false);
            maps.set_has_roughness(false);
            maps.set_has_ao(false);
            window.global::<PackingApi>().set_has_preview(false);
            window.global::<Nav>().set_tab(0);
            window.global::<TileableApi>().set_status(locale.tr("tileable-status-sent").into());
        }
    });
}
