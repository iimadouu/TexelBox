use std::sync::Arc;
use std::time::Instant;
use image::RgbaImage;
use slint::{ComponentHandle, Weak};
use tbx_core::maps::{self, AoParams, HeightParams, HeightSource, MapSetParams, NormalKernel, NormalParams, RoughnessParams};
use tbx_entitlements::Capability;
use tbx_locale::LocaleManager;
use tbx_state::{AppState, Stage};
use crate::session_config::{self, SessionConfig};
use crate::{L10n, MainWindow, MapsApi, PackingApi, Session};
pub(crate) const FREE_MAX_SIDE: u32 = 1024;
pub(crate) fn rgba_to_slint(img: &RgbaImage) -> slint::Image {
    let (w, h) = img.dimensions();
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(img.as_raw(), w, h);
    slint::Image::from_rgba8(buffer)
}
pub fn sync_entitlements(window: &MainWindow, state: &AppState) {
    let gate = &state.gate;
    let api = window.global::<MapsApi>();
    api.set_lock_sliders(gate.is_locked(Capability::MapsFullSliderControl));
    api.set_lock_hires(gate.is_locked(Capability::MapsHighResolution));
    let session = window.global::<Session>();
    let l10n = window.global::<L10n>();
    let plan = gate.plan();
    session.set_plan(match plan {
        tbx_entitlements::Plan::Free => 0,
        tbx_entitlements::Plan::Pro => 1,
        tbx_entitlements::Plan::Trial => 2,
    });
    let label = if !gate.has_valid_token() {
        l10n.get_license_none()
    } else if plan == tbx_entitlements::Plan::Trial {
        l10n.get_license_plan_trial()
    } else if plan == tbx_entitlements::Plan::Pro {
        l10n.get_license_plan_pro()
    } else {
        l10n.get_license_plan_free()
    };
    session.set_plan_label(label);
    session.set_trial_expires("".into());
}
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>, session: Arc<std::sync::Mutex<SessionConfig>>) {
    let weak = window.as_weak().clone();
    if let Some(last_path) = session.lock().unwrap_or_else(|e| e.into_inner()).last_source.clone() {
        if last_path.exists() {
            if let Ok(img) = tbx_core::io::load(&last_path) {
                let name = last_path.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "source".into());
                let (w, h) = img.dimensions();
                let img_arc = Arc::new(img.clone());
                state.update(Stage::Source, |p| {
                    p.source = Some(tbx_state::ImageSlot {
                        image: img_arc,
                        name: name.clone(),
                        origin: last_path.display().to_string(),
                    });
                });
                let api = window.global::<MapsApi>();
                api.set_source_image(rgba_to_slint(&img));
                api.set_source_name(name.into());
                api.set_has_source(true);
                api.set_resolution_label(format!("{w}×{h} px").into());
            }
        }
    }
    window.global::<MapsApi>().on_open_source({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        let session = session.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<MapsApi>();
            let picked = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
                .pick_file();
            let Some(path) = picked else { return };
            match tbx_core::io::load(&path) {
                Ok(mut img) => {
                    let capped = if state.gate.is_locked(Capability::MapsHighResolution)
                        && img.dimensions().0.max(img.dimensions().1) > FREE_MAX_SIDE
                    {
                        img = tbx_core::io::downscale_to_fit(&img, FREE_MAX_SIDE);
                        true
                    } else {
                        false
                    };
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "source".into());
                    let (w, h) = img.dimensions();
                    state.update(Stage::Source, |p| {
                        p.source = Some(tbx_state::ImageSlot {
                            image: Arc::new(img.clone()),
                            name: name.clone(),
                            origin: path.display().to_string(),
                        });
                        p.maps.outputs = None;
                    });
                    api.set_source_image(rgba_to_slint(&img));
                    api.set_source_name(name.into());
                    api.set_has_source(true);
                    api.set_resolution_label(format!("{w}×{h} px").into());
                    api.set_has_normal(false);
                    api.set_has_height(false);
                    api.set_has_roughness(false);
                    api.set_has_ao(false);
                    window.global::<PackingApi>().set_has_preview(false);
                    {
                        let mut cfg = session.lock().unwrap_or_else(|e| e.into_inner());
                        cfg.last_source = Some(path.clone());
                        if let Some(folder) = path.parent() {
                            session_config::set_last_folder(&mut cfg, "maps", folder.to_path_buf());
                        } else {
                            session_config::save(&cfg);
                        }
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
    window.global::<MapsApi>().on_generate({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<MapsApi>();
            if api.get_busy() {
                return;
            }
            let source = {
                let p = state.project.read().unwrap_or_else(|e| e.into_inner());
                p.source.as_ref().map(|s| s.image.clone())
            };
            let Some(source) = source else {
                api.set_status(locale.tr("maps-status-need-source").into());
                return;
            };
            let capped = state.gate.is_locked(Capability::MapsHighResolution)
                && source.dimensions().0.max(source.dimensions().1) > FREE_MAX_SIDE;
            let source = if capped {
                Arc::new(tbx_core::io::downscale_to_fit(&source, FREE_MAX_SIDE))
            } else {
                source
            };
            let sliders_locked = state.gate.is_locked(Capability::MapsFullSliderControl);
            let mut params = MapSetParams::default();
            if !sliders_locked {
                params.height = HeightParams {
                    source: match api.get_height_source_channel() {
                        1 => HeightSource::Red,
                        2 => HeightSource::Green,
                        3 => HeightSource::Blue,
                        4 => HeightSource::Alpha,
                        _ => HeightSource::Luminance,
                    },
                    contrast: api.get_contrast(),
                    brightness: api.get_brightness(),
                    blur_radius: api.get_blur_radius(),
                    invert: api.get_invert_height(),
                };
                params.normal = NormalParams {
                    strength: api.get_normal_strength(),
                    auto_strength: api.get_normal_auto_strength(),
                    detail_enhance: api.get_detail_enhance(),
                    kernel: if api.get_normal_kernel() == 0 { NormalKernel::K3 } else { NormalKernel::K5 },
                    wrap_edges: api.get_wrap_edges(),
                };
                params.roughness = RoughnessParams {
                    radius: api.get_rough_radius(),
                    amount: api.get_rough_amount(),
                    curve: api.get_rough_curve(),
                    invert: api.get_rough_invert(),
                };
                params.ao = AoParams {
                    radius: api.get_ao_radius(),
                    intensity: api.get_ao_intensity(),
                };
            }
            params.enable_height = api.get_enable_height();
            params.enable_normal = api.get_enable_normal();
            params.enable_roughness = api.get_enable_roughness();
            params.enable_ao = api.get_enable_ao();
            api.set_busy(true);
            api.set_status(locale.tr("common-busy").into());
            let start = Instant::now();
            let state_w = state.clone();
            let locale_w = locale.clone();
            let weak_w = weak.clone();
            std::thread::spawn(move || {
                let outputs = maps::generate_all(&source, &params);
                let secs = start.elapsed().as_secs_f32();
                let count = outputs.generated_count();
                let state = state_w.clone();
                let locale = locale_w.clone();
                let weak = weak_w.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak.upgrade() else { return };
                    let api = window.global::<MapsApi>();
                    state.update(Stage::Maps, |p| {
                        p.maps.params = params;
                        p.maps.outputs = Some(outputs.clone());
                    });
                    if let Some(n) = &outputs.normal {
                        api.set_out_normal(rgba_to_slint(n));
                        api.set_has_normal(true);
                    } else {
                        api.set_has_normal(false);
                    }
                    if let Some(h) = &outputs.height {
                        api.set_out_height(rgba_to_slint(&h.to_rgba()));
                        api.set_has_height(true);
                    } else {
                        api.set_has_height(false);
                    }
                    if let Some(r) = &outputs.roughness {
                        api.set_out_roughness(rgba_to_slint(&r.to_rgba()));
                        api.set_has_roughness(true);
                    } else {
                        api.set_has_roughness(false);
                    }
                    if let Some(a) = &outputs.ao {
                        api.set_out_ao(rgba_to_slint(&a.to_rgba()));
                        api.set_has_ao(true);
                    } else {
                        api.set_has_ao(false);
                    }
                    api.set_busy(false);
                    api.set_status(
                        locale
                            .tr_args(
                                "maps-status-generated",
                                &[
                                    ("count", count.to_string().as_str().into()),
                                    ("secs", format!("{secs:.2}").as_str().into()),
                                ],
                            )
                            .into(),
                    );
                    if capped {
                        api.set_status(
                            locale
                                .tr_args(
                                    "maps-resolution-capped",
                                    &[("max", tbx_locale::num(FREE_MAX_SIDE as f64))],
                                )
                                .into(),
                        );
                    }
                });
            });
        }
    });
    window.global::<MapsApi>().on_save_map({
        let state = state.clone();
        let locale = locale.clone();
        let weak = weak.clone();
        move |which: i32| {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<MapsApi>();
            let (img, default_name) = {
                let p = state.project.read().unwrap_or_else(|e| e.into_inner());
                let name = p.source.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| "map".into());
                let Some(outs) = p.maps.outputs.as_ref() else {
                    api.set_status(locale.tr("maps-status-need-generate").into());
                    return;
                };
                let pair = match which {
                    0 => outs.normal.as_ref().cloned().map(|i| (i, "normal")),
                    1 => outs.height.as_ref().map(|g| (g.to_rgba(), "height")),
                    2 => outs.roughness.as_ref().map(|g| (g.to_rgba(), "roughness")),
                    3 => outs.ao.as_ref().map(|g| (g.to_rgba(), "ao")),
                    _ => None,
                };
                let Some((img, suffix)) = pair else {
                    api.set_status(locale.tr("maps-status-need-generate").into());
                    return;
                };
                (img, format!("{name}_{suffix}.png"))
            };
            let picked = rfd::FileDialog::new()
                .set_file_name(default_name)
                .add_filter("PNG", &["png"])
                .add_filter("TGA", &["tga"])
                .save_file();
            let Some(path) = picked else { return };
            match tbx_core::io::save(&path, &img) {
                Ok(()) => api.set_status(
                    locale
                        .tr_args("maps-status-saved", &[("path", path.display().to_string().as_str().into())])
                        .into(),
                ),
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
    window.global::<MapsApi>().on_quick_export({
        let state = state.clone();
        let locale = locale.clone();
        let session = session.clone();
        let weak = weak.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<MapsApi>();
            let (outputs, source_name, last_folder) = {
                let p = state.project.read().unwrap_or_else(|e| e.into_inner());
                let cfg = session.lock().unwrap_or_else(|e| e.into_inner());
                let folder = session_config::last_folder(&cfg, "maps");
                let name = p.source.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| "map".into());
                let outs = p.maps.outputs.clone();
                (outs, name, folder)
            };
            let Some(outputs) = outputs else {
                api.set_status(locale.tr("maps-status-need-generate").into());
                return;
            };
            let folder = if let Some(f) = last_folder {
                f
            } else {
                let picked = rfd::FileDialog::new().pick_folder();
                let Some(f) = picked else { return };
                f
            };
            let mut saved = 0usize;
            let mut failed = false;
            let pairs: &[(&str, Option<image::RgbaImage>)] = &[
                ("normal",    outputs.normal.clone()),
                ("height",    outputs.height.as_ref().map(|g| g.to_rgba())),
                ("roughness", outputs.roughness.as_ref().map(|g| g.to_rgba())),
                ("ao",        outputs.ao.as_ref().map(|g| g.to_rgba())),
            ];
            for (suffix, img_opt) in pairs {
                if let Some(img) = img_opt {
                    let path = folder.join(format!("{source_name}_{suffix}.png"));
                    if tbx_core::io::save(&path, &img).is_ok() {
                        saved += 1;
                    } else {
                        failed = true;
                    }
                }
            }
            {
                let mut cfg = session.lock().unwrap_or_else(|e| e.into_inner());
                session_config::set_last_folder(&mut cfg, "maps", folder);
            }
            if failed {
                api.set_status(locale.tr("common-error").into());
            } else {
                api.set_status(
                    locale.tr_args("maps-status-quick-exported", &[("count", tbx_locale::num(saved as f64))]).into(),
                );
            }
        }
    });
    window.global::<MapsApi>().on_send_to_packer({
        let state = state.clone();
        let locale = locale.clone();
        let weak: Weak<MainWindow> = weak.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            crate::packing_panel::apply_maps_handoff(&window, state.clone(), locale.clone());
        }
    });
}
