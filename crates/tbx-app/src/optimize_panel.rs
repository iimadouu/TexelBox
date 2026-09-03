use std::sync::{Arc, Mutex};
use image::RgbaImage;
use slint::ComponentHandle;
use tbx_core::io;
use tbx_core::optimize::{self, DdsCompression, OptimizeParams, PotSnap, Resampling};
use tbx_entitlements::Capability;
use tbx_locale::LocaleManager;
use tbx_state::AppState;
use crate::{maps_panel::rgba_to_slint, OptimizeApi, MainWindow};
struct CustomSource {
    path: std::path::PathBuf,
    image: RgbaImage,
}
pub fn sync_entitlements(window: &MainWindow, state: &AppState) {
    let api = window.global::<OptimizeApi>();
    api.set_lock_dds(state.gate.is_locked(Capability::OptimizeDdsCompression));
    api.set_lock_resampling(state.gate.is_locked(Capability::OptimizeResamplingChoice));
    api.set_lock_template(state.gate.is_locked(Capability::OptimizeBatchTemplateExport));
}
fn resolve_source(
    state: &AppState,
    source_index: i32,
    custom: &Option<CustomSource>,
) -> Option<(String, Arc<RgbaImage>)> {
    let project = state.project.read().unwrap_or_else(|e| e.into_inner());
    match source_index {
        0 => custom.as_ref().map(|c| (c.path.file_stem().unwrap_or_default().to_string_lossy().into_owned(), Arc::new(c.image.clone()))),
        1 => project.source.as_ref().map(|s| (s.name.clone(), Arc::clone(&s.image))),
        2 => project.tileable.as_ref().map(|s| (s.name.clone(), Arc::clone(&s.image))),
        3 => project.maps.outputs.as_ref().and_then(|o| o.normal.as_ref().map(|i| ("normal".into(), Arc::new(i.clone())))),
        4 => project.maps.outputs.as_ref().and_then(|o| o.height.as_ref().map(|g| ("height".into(), Arc::new(g.to_rgba())))),
        5 => project.maps.outputs.as_ref().and_then(|o| o.roughness.as_ref().map(|g| ("roughness".into(), Arc::new(g.to_rgba())))),
        6 => project.maps.outputs.as_ref().and_then(|o| o.ao.as_ref().map(|g| ("ao".into(), Arc::new(g.to_rgba())))),
        7 => project.packed.as_ref().map(|s| (s.name.clone(), Arc::clone(&s.image))),
        8 => project.atlas.as_ref().map(|s| (s.name.clone(), Arc::clone(&s.image))),
        _ => None,
    }
}
fn collect_project_outputs(project: &tbx_state::ProjectState) -> Vec<(String, Arc<RgbaImage>)> {
    let mut out = Vec::new();
    if let Some(s) = &project.source {
        out.push((s.name.clone(), Arc::clone(&s.image)));
    }
    if let Some(t) = &project.tileable {
        out.push((t.name.clone(), Arc::clone(&t.image)));
    }
    if let Some(o) = &project.maps.outputs {
        if let Some(n) = &o.normal { out.push(("normal".into(), Arc::new(n.clone()))); }
        if let Some(h) = &o.height { out.push(("height".into(), Arc::new(h.to_rgba()))); }
        if let Some(r) = &o.roughness { out.push(("roughness".into(), Arc::new(r.to_rgba()))); }
        if let Some(a) = &o.ao { out.push(("ao".into(), Arc::new(a.to_rgba()))); }
    }
    if let Some(p) = &project.packed {
        out.push((p.name.clone(), Arc::clone(&p.image)));
    }
    if let Some(a) = &project.atlas {
        out.push((a.name.clone(), Arc::clone(&a.image)));
    }
    out
}
fn build_params(api: &OptimizeApi, state: &AppState) -> OptimizeParams {
    let resampling = if state.gate.is_locked(Capability::OptimizeResamplingChoice) {
        Resampling::Bilinear
    } else {
        match api.get_resampling_index() {
            0 => Resampling::Bilinear,
            1 => Resampling::Bicubic,
            _ => Resampling::Lanczos3,
        }
    };
    let size_presets = [64, 128, 256, 512, 1024, 2048, 4096];
    let target_size = match api.get_size_index() {
        0 => None,
        idx if idx > 0 && (idx as usize) <= size_presets.len() => Some(size_presets[idx as usize - 1]),
        _ => None,
    };
    let pot_snap = match api.get_snap_index() {
        0 => PotSnap::None,
        1 => PotSnap::Nearest,
        2 => PotSnap::Up,
        3 => PotSnap::Down,
        _ => PotSnap::None,
    };
    OptimizeParams { target_size, pot_snap, resampling, dds: None }
}
fn render_export_name(template: &str, name: &str, map: &str, ext: &str) -> String {
    let t = if map.is_empty() {
        template.replace("_{map}", "").replace("-{map}", "").replace(" {map}", "")
    } else {
        template.to_string()
    };
    optimize::render_name(&t, name, map, ext)
}
fn chosen_format(api: &OptimizeApi, state: &AppState) -> (io::ImageFormat, &'static str, bool) {
    let dds_locked = state.gate.is_locked(Capability::OptimizeDdsCompression);
    match api.get_format_index() {
        1 => (io::ImageFormat::Tga, "tga", false),
        2 if !dds_locked => (io::ImageFormat::Dds, "dds", false),
        _ => (io::ImageFormat::Png, "png", api.get_format_index() == 2 && dds_locked),
    }
}
fn bc_from_index(i: i32) -> DdsCompression {
    match i {
        0 => DdsCompression::Bc1,
        2 => DdsCompression::Bc5,
        3 => DdsCompression::Bc7,
        _ => DdsCompression::Bc3,
    }
}
fn write_image(
    path: &std::path::Path,
    img: &RgbaImage,
    fmt: io::ImageFormat,
    bc: DdsCompression,
    fast: bool,
) -> Result<(), io::CoreError> {
    if fmt == io::ImageFormat::Dds {
        let bytes = optimize::encode_dds(img, bc, fast)?;
        std::fs::write(path, bytes).map_err(|e| io::CoreError::Write(e.to_string()))
    } else {
        io::save_with(path, img, fmt, None)
    }
}
fn estimate_size_bytes(img: &RgbaImage, fmt: io::ImageFormat, bc: DdsCompression) -> u64 {
    let (w, h) = img.dimensions();
    let pixels = w as u64 * h as u64;
    match fmt {
        io::ImageFormat::Png => pixels * 2 / 8 * 8,
        io::ImageFormat::Tga => pixels * 4,
        io::ImageFormat::Dds => {
            let bpp: u64 = match bc {
                DdsCompression::Bc1 => 4,
                DdsCompression::Bc3 | DdsCompression::Bc5 | DdsCompression::Bc7 => 8,
            };
            pixels * bpp / 8
        }
    }
}
fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("~{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("~{} KB", (bytes + 1023) / 1024)
    }
}
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let weak = window.as_weak().clone();
    let custom: Arc<Mutex<Option<CustomSource>>> = Arc::new(Mutex::new(None));
    window.global::<OptimizeApi>().on_open_custom({
        let weak = weak.clone();
        let locale = locale.clone();
        let _state = state.clone();
        let custom = custom.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<OptimizeApi>();
            let picked = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
                .pick_file();
            let Some(path) = picked else { return };
            match io::load(&path) {
                Ok(img) => {
                    let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                    *custom.lock().unwrap_or_else(|e| e.into_inner()) = Some(CustomSource { path, image: img.clone() });
                    api.set_source_index(0);
                    api.set_source_info(
                        locale.tr_args("optimize-src-custom", &[("name", name.into())]).into(),
                    );
                    api.set_preview(rgba_to_slint(&img));
                    api.set_has_preview(true);
                    api.set_preview_info(
                        locale.tr_args(
                            "common-resolution",
                            &[("w", img.width().to_string().as_str().into()), ("h", img.height().to_string().as_str().into())],
                        ).into(),
                    );
                }
                Err(e) => {
                    api.set_status(format!("{}: {}", locale.tr("common-error"), e).into());
                }
            }
        }
    });
    window.global::<OptimizeApi>().on_update_preview({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        let custom = custom.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<OptimizeApi>();
            let source_idx = api.get_source_index();
            let Some((_name, img)) = resolve_source(&state, source_idx, &custom.lock().unwrap_or_else(|e| e.into_inner())) else {
                api.set_has_preview(false);
                api.set_preview_info(locale.tr("common-no-image").into());
                return;
            };
            let params = build_params(&api, &state);
            let out = optimize::resize(&img, &params);
            api.set_preview(rgba_to_slint(&out));
            api.set_has_preview(true);
            let (tw, th) = out.dimensions();
            api.set_preview_info(
                locale.tr_args(
                    "common-resolution",
                    &[("w", tw.to_string().as_str().into()), ("h", th.to_string().as_str().into())],
                ).into(),
            );
            let (fmt, fmt_label, _) = chosen_format(&api, &state);
            let bc = bc_from_index(api.get_bc_index());
            let est = estimate_size_bytes(&out, fmt, bc);
            let fmt_name = match fmt {
                io::ImageFormat::Png => "PNG",
                io::ImageFormat::Tga => "TGA",
                io::ImageFormat::Dds => "DDS",
            };
            let _ = fmt_label;
            api.set_size_estimate(format!("{} ({})", format_size(est), fmt_name).into());
            api.set_status(locale.tr("optimize-status-updated").into());
        }
    });
    window.global::<OptimizeApi>().on_export_selected({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        let custom = custom.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<OptimizeApi>();
            let source_idx = api.get_source_index();
            let Some((name, img)) = resolve_source(&state, source_idx, &custom.lock().unwrap_or_else(|e| e.into_inner())) else {
                api.set_status(locale.tr("common-no-image").into());
                return;
            };
            let params = build_params(&api, &state);
            let out = optimize::resize(&img, &params);
            let (fmt, ext, forced_png) = chosen_format(&api, &state);
            let default_name = format!("{name}.{ext}");
            let dds_locked = state.gate.is_locked(Capability::OptimizeDdsCompression);
            let mut dlg = rfd::FileDialog::new()
                .set_file_name(default_name)
                .add_filter("PNG", &["png"])
                .add_filter("TGA", &["tga"]);
            if !dds_locked {
                dlg = dlg.add_filter("DDS", &["dds"]);
            }
            let Some(path) = dlg.save_file() else { return };
            let final_fmt = io::ImageFormat::from_path(&path).unwrap_or(fmt);
            if final_fmt == io::ImageFormat::Dds && dds_locked {
                api.set_status(locale.tr("optimize-locked-dds").into());
                return;
            }
            match write_image(&path, &out, final_fmt, bc_from_index(api.get_bc_index()), api.get_dds_quality() < 0.5) {
                Ok(()) => {
                    let mut status = locale.tr_args(
                        "optimize-status-exported",
                        &[("path", path.display().to_string().as_str().into())],
                    );
                    if forced_png {
                        status = format!("{status} — {}", locale.tr("optimize-locked-dds"));
                    }
                    api.set_status(status.into());
                }
                Err(e) => api.set_status(format!("{}: {}", locale.tr("common-error"), e).into()),
            }
        }
    });
    window.global::<OptimizeApi>().on_export_all({
        let weak = weak.clone();
        let locale = locale.clone();
        let state = state.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<OptimizeApi>();
            if api.get_busy() {
                return;
            }
            let outputs = {
                let project = state.project.read().unwrap_or_else(|e| e.into_inner());
                collect_project_outputs(&project)
            };
            if outputs.is_empty() {
                api.set_status(locale.tr("common-no-image").into());
                return;
            }
            let params = build_params(&api, &state);
            let (fmt, ext, forced_png) = chosen_format(&api, &state);
            let template = api.get_name_template().to_string();
            let bc = bc_from_index(api.get_bc_index());
            let fast = api.get_dds_quality() < 0.5;
            let folder = rfd::FileDialog::new().pick_folder();
            let Some(out_dir) = folder else { return };
            api.set_busy(true);
            api.set_status(locale.tr("common-busy").into());
            let weak_w = weak.clone();
            let locale_w = locale.clone();
            std::thread::spawn(move || {
                let mut count = 0usize;
                let mut error: Option<String> = None;
                let mut used = std::collections::HashSet::<String>::new();
                for (name, img) in outputs {
                    let out = optimize::resize(&img, &params);
                    let mut file_name =
                        optimize::sanitize_filename(&render_export_name(&template, &name, "", ext));
                    if !used.insert(file_name.clone()) {
                        let (stem, ext_part) = file_name.rsplit_once('.').unwrap_or((&file_name, ext));
                        let mut n = 2u32;
                        loop {
                            let candidate = format!("{stem}_{n}.{ext_part}");
                            if used.insert(candidate.clone()) {
                                file_name = candidate;
                                break;
                            }
                            n += 1;
                        }
                    }
                    let path = out_dir.join(&file_name);
                    if let Err(e) = write_image(&path, &out, fmt, bc, fast) {
                        error = Some(format!("{}: {}", path.display(), e));
                        break;
                    }
                    count += 1;
                }
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak_w.upgrade() else { return };
                    let api = window.global::<OptimizeApi>();
                    api.set_busy(false);
                    if let Some(e) = error {
                        api.set_status(
                            locale_w
                                .tr_args("optimize-status-error", &[("error", e.as_str().into())])
                                .into(),
                        );
                    } else {
                        let mut status = locale_w.tr_args(
                            "optimize-status-exported-batch",
                            &[
                                ("count", tbx_locale::num(count as f64)),
                                ("path", out_dir.display().to_string().as_str().into()),
                            ],
                        );
                        if forced_png {
                            status = format!("{status} — {}", locale_w.tr("optimize-locked-dds"));
                        }
                        api.set_status(status.into());
                    }
                });
            });
        }
    });
}
