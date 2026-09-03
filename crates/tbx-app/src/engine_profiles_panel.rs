use std::sync::Arc;
use image::RgbaImage;
use slint::ComponentHandle;
use tbx_core::{buffer::GrayF32, io, maps};
use tbx_core::maps::MapSetParams;
use tbx_entitlements::Capability;
use tbx_locale::LocaleManager;
use tbx_state::{AppState, ImageSlot, Stage};
use crate::maps_panel::rgba_to_slint;
use crate::{EngineProfilesApi, MainWindow, MapsApi};
pub fn sync_entitlements(window: &MainWindow, state: &AppState) {
    let api = window.global::<EngineProfilesApi>();
    api.set_lock_profiles(state.gate.is_locked(Capability::EngineExportProfiles));
}
fn pack3(
    r: Option<&GrayF32>,
    g: Option<&GrayF32>,
    b: Option<&GrayF32>,
    a_val: u8,
    w: u32,
    h: u32,
) -> RgbaImage {
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let rv = r.map(|c| c.at(x.min(c.width - 1), y.min(c.height - 1))).unwrap_or(0.0);
            let gv = g.map(|c| c.at(x.min(c.width - 1), y.min(c.height - 1))).unwrap_or(0.0);
            let bv = b.map(|c| c.at(x.min(c.width - 1), y.min(c.height - 1))).unwrap_or(0.0);
            out.put_pixel(x, y, image::Rgba([
                (rv.clamp(0.0, 1.0) * 255.0) as u8,
                (gv.clamp(0.0, 1.0) * 255.0) as u8,
                (bv.clamp(0.0, 1.0) * 255.0) as u8,
                a_val,
            ]));
        }
    }
    out
}
fn pack4(
    r: Option<&GrayF32>,
    g: Option<&GrayF32>,
    b: Option<&GrayF32>,
    a: Option<&GrayF32>,
    w: u32,
    h: u32,
) -> RgbaImage {
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let rv = r.map(|c| c.at(x.min(c.width - 1), y.min(c.height - 1))).unwrap_or(0.0);
            let gv = g.map(|c| c.at(x.min(c.width - 1), y.min(c.height - 1))).unwrap_or(0.0);
            let bv = b.map(|c| c.at(x.min(c.width - 1), y.min(c.height - 1))).unwrap_or(0.0);
            let av = a.map(|c| c.at(x.min(c.width - 1), y.min(c.height - 1))).unwrap_or(1.0);
            out.put_pixel(x, y, image::Rgba([
                (rv.clamp(0.0, 1.0) * 255.0) as u8,
                (gv.clamp(0.0, 1.0) * 255.0) as u8,
                (bv.clamp(0.0, 1.0) * 255.0) as u8,
                (av.clamp(0.0, 1.0) * 255.0) as u8,
            ]));
        }
    }
    out
}
fn generate_maps(source: &RgbaImage) -> maps::MapOutputs {
    let params = MapSetParams::default();
    maps::generate_all(source, &params)
}
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let weak = window.as_weak().clone();
    window.global::<EngineProfilesApi>().on_open_source({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<EngineProfilesApi>();
            let picked = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
                .pick_file();
            let Some(path) = picked else { return };
            match io::load(&path) {
                Ok(img) => {
                    let name = path.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "source".into());
                    let img_arc = Arc::new(img.clone());
                    state.update(Stage::Source, |p| {
                        p.source = Some(ImageSlot {
                            image: img_arc.clone(),
                            name: name.clone(),
                            origin: path.display().to_string(),
                        });
                        p.maps.outputs = None;
                    });
                    let (w, h) = img.dimensions();
                    let maps_api = window.global::<MapsApi>();
                    maps_api.set_source_image(rgba_to_slint(&img));
                    maps_api.set_source_name(name.clone().into());
                    maps_api.set_has_source(true);
                    maps_api.set_resolution_label(format!("{w}×{h} px").into());
                    api.set_source_name(name.into());
                    api.set_has_source(true);
                    api.set_status("".into());
                }
                Err(e) => {
                    api.set_status(format!("{}: {}", locale.tr("common-error"), e).into());
                }
            }
        }
    });
    window.global::<EngineProfilesApi>().on_export_unreal5({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<EngineProfilesApi>();
            if state.gate.is_locked(Capability::EngineExportProfiles) {
                api.set_status(locale.tr("engine-locked").into());
                return;
            }
            if api.get_busy() { return; }
            let (source, name) = {
                let p = state.project.read().unwrap_or_else(|e| e.into_inner());
                match p.source.as_ref() {
                    Some(s) => (Arc::clone(&s.image), s.name.clone()),
                    None => { api.set_status(locale.tr("engine-need-maps").into()); return; }
                }
            };
            let folder = match rfd::FileDialog::new().pick_folder() {
                Some(f) => f,
                None => return,
            };
            api.set_busy(true);
            api.set_status(locale.tr("common-busy").into());
            let weak_w = weak.clone();
            let locale_w = locale.clone();
            let state_w = state.clone();
            std::thread::spawn(move || {
                let outputs = generate_maps(&source);
                let (w, h) = source.dimensions();
                let ao = outputs.ao.as_ref();
                let rough = outputs.roughness.as_ref();
                let owned: Vec<(String, RgbaImage)> = vec![
                    (format!("T_{name}_D.png"), (*source).clone()),
                    (format!("T_{name}_ORM.png"), pack3(ao, rough, None, 255, w, h)),
                    (format!("T_{name}_N.png"), outputs.normal.clone().unwrap_or_else(|| RgbaImage::new(w, h))),
                ];
                let (count, err) = {
                    let mut c = 0usize;
                    let mut e: Option<String> = None;
                    for (fname, img) in &owned {
                        let path = folder.join(fname);
                        if let Err(err) = io::save(&path, img) {
                            e = Some(format!("{}: {err}", path.display()));
                            break;
                        }
                        c += 1;
                    }
                    (c, e)
                };
                state_w.update(Stage::Maps, |p| { p.maps.outputs = Some(outputs); });
                let folder_str = folder.display().to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak_w.upgrade() else { return };
                    let api = window.global::<EngineProfilesApi>();
                    api.set_busy(false);
                    if let Some(e) = err {
                        api.set_status(format!("{}: {e}", locale_w.tr("common-error")).into());
                    } else {
                        api.set_status(
                            locale_w.tr_args("engine-status-done", &[
                                ("count", tbx_locale::num(count as f64)),
                                ("path", folder_str.as_str().into()),
                            ]).into(),
                        );
                    }
                });
            });
        }
    });
    window.global::<EngineProfilesApi>().on_export_unity_hdrp({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<EngineProfilesApi>();
            if state.gate.is_locked(Capability::EngineExportProfiles) {
                api.set_status(locale.tr("engine-locked").into());
                return;
            }
            if api.get_busy() { return; }
            let (source, name) = {
                let p = state.project.read().unwrap_or_else(|e| e.into_inner());
                match p.source.as_ref() {
                    Some(s) => (Arc::clone(&s.image), s.name.clone()),
                    None => { api.set_status(locale.tr("engine-need-maps").into()); return; }
                }
            };
            let folder = match rfd::FileDialog::new().pick_folder() {
                Some(f) => f,
                None => return,
            };
            api.set_busy(true);
            api.set_status(locale.tr("common-busy").into());
            let weak_w = weak.clone();
            let locale_w = locale.clone();
            let state_w = state.clone();
            std::thread::spawn(move || {
                let outputs = generate_maps(&source);
                let (w, h) = source.dimensions();
                let ao = outputs.ao.as_ref();
                let rough = outputs.roughness.as_ref();
                let smooth: Option<GrayF32> = rough.map(|r| {
                    let mut s = r.clone();
                    for v in s.data_mut().iter_mut() { *v = 1.0 - *v; }
                    s
                });
                let mask = pack4(None, ao, None, smooth.as_ref(), w, h);
                let normal = outputs.normal.clone().unwrap_or_else(|| RgbaImage::new(w, h));
                let owned: Vec<(String, RgbaImage)> = vec![
                    (format!("{name}_Albedo.png"), (*source).clone()),
                    (format!("{name}_MaskMap.png"), mask),
                    (format!("{name}_NormalMap.png"), normal),
                ];
                let (count, err) = {
                    let mut c = 0usize;
                    let mut e: Option<String> = None;
                    for (fname, img) in &owned {
                        let path = folder.join(fname);
                        if let Err(err) = io::save(&path, img) {
                            e = Some(format!("{}: {err}", path.display()));
                            break;
                        }
                        c += 1;
                    }
                    (c, e)
                };
                state_w.update(Stage::Maps, |p| { p.maps.outputs = Some(outputs); });
                let folder_str = folder.display().to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak_w.upgrade() else { return };
                    let api = window.global::<EngineProfilesApi>();
                    api.set_busy(false);
                    if let Some(e) = err {
                        api.set_status(format!("{}: {e}", locale_w.tr("common-error")).into());
                    } else {
                        api.set_status(
                            locale_w.tr_args("engine-status-done", &[
                                ("count", tbx_locale::num(count as f64)),
                                ("path", folder_str.as_str().into()),
                            ]).into(),
                        );
                    }
                });
            });
        }
    });
    window.global::<EngineProfilesApi>().on_export_godot4({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<EngineProfilesApi>();
            if state.gate.is_locked(Capability::EngineExportProfiles) {
                api.set_status(locale.tr("engine-locked").into());
                return;
            }
            if api.get_busy() { return; }
            let (source, name) = {
                let p = state.project.read().unwrap_or_else(|e| e.into_inner());
                match p.source.as_ref() {
                    Some(s) => (Arc::clone(&s.image), s.name.clone()),
                    None => { api.set_status(locale.tr("engine-need-maps").into()); return; }
                }
            };
            let folder = match rfd::FileDialog::new().pick_folder() {
                Some(f) => f,
                None => return,
            };
            api.set_busy(true);
            api.set_status(locale.tr("common-busy").into());
            let weak_w = weak.clone();
            let locale_w = locale.clone();
            let state_w = state.clone();
            std::thread::spawn(move || {
                let outputs = generate_maps(&source);
                let (w, h) = source.dimensions();
                let normal = outputs.normal.clone().unwrap_or_else(|| RgbaImage::new(w, h));
                let roughness = outputs.roughness.as_ref().map(|g| g.to_rgba()).unwrap_or_else(|| RgbaImage::new(w, h));
                let ao = outputs.ao.as_ref().map(|g| g.to_rgba()).unwrap_or_else(|| RgbaImage::new(w, h));
                let owned: Vec<(String, RgbaImage)> = vec![
                    (format!("{name}_albedo.png"), (*source).clone()),
                    (format!("{name}_normal.png"), normal),
                    (format!("{name}_roughness.png"), roughness),
                    (format!("{name}_ao.png"), ao),
                ];
                let (count, err) = {
                    let mut c = 0usize;
                    let mut e: Option<String> = None;
                    for (fname, img) in &owned {
                        let path = folder.join(fname);
                        if let Err(err) = io::save(&path, img) {
                            e = Some(format!("{}: {err}", path.display()));
                            break;
                        }
                        c += 1;
                    }
                    (c, e)
                };
                state_w.update(Stage::Maps, |p| { p.maps.outputs = Some(outputs); });
                let folder_str = folder.display().to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak_w.upgrade() else { return };
                    let api = window.global::<EngineProfilesApi>();
                    api.set_busy(false);
                    if let Some(e) = err {
                        api.set_status(format!("{}: {e}", locale_w.tr("common-error")).into());
                    } else {
                        api.set_status(
                            locale_w.tr_args("engine-status-done", &[
                                ("count", tbx_locale::num(count as f64)),
                                ("path", folder_str.as_str().into()),
                            ]).into(),
                        );
                    }
                });
            });
        }
    });
}
