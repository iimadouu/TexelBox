
#![allow(clippy::doc_overindented_list_items, clippy::doc_lazy_continuation)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::Arc;
use tbx_locale::LocaleManager;
use tbx_state::AppState;
mod atlas_panel;
mod batch_panel;
mod engine_profiles_panel;
mod i18n;
mod license_net;
mod maps_panel;
mod optimize_panel;
mod packing_panel;
mod presets;
mod presets_panel;
mod preview_panel;
mod session_config;
mod settings_panel;
mod tileable_panel;
mod update_net;
slint::include_modules!();
pub(crate) fn sync_all(window: &MainWindow, state: &AppState) {
    maps_panel::sync_entitlements(window, state);
    tileable_panel::sync_entitlements(window, state);
    packing_panel::sync_entitlements(window, state);
    atlas_panel::sync_entitlements(window, state);
    optimize_panel::sync_entitlements(window, state);
    preview_panel::sync_entitlements(window, state);
    batch_panel::sync_entitlements(window, state);
    presets_panel::sync_entitlements(window, state);
    engine_profiles_panel::sync_entitlements(window, state);
}
fn check_trial_expiry(window: &MainWindow, state: &AppState, _locale: &LocaleManager) {
    let current_plan = state.gate.plan();
    if current_plan == tbx_entitlements::Plan::Pro {
        return;
    }
    if license_net::is_trial_expired(None) {
        let _ = window.global::<Session>().set_trial_expired(true);
    }
}
fn update_trial_warning(window: &MainWindow, state: &AppState) {
    let is_trial = state.gate.plan() == tbx_entitlements::Plan::Trial;
    let now = chrono::Utc::now().timestamp();
    let show_warning = if is_trial {
        license_net::load_cache()
            .and_then(|c| c.trial_expires_at)
            .map(|expires_at| {
                let remaining = expires_at - now;
                remaining > 0 && remaining < 3600
            })
            .unwrap_or(false)
    } else {
        false
    };
    let _ = window.global::<Session>().set_trial_warning(show_warning);
    if is_trial {
        if let Some(expires_at) = license_net::load_cache().and_then(|c| c.trial_expires_at) {
            let remaining = expires_at - now;
            if remaining <= 0 {
                let _ = window.global::<Session>().set_trial_expires("".into());
            } else {
                let hours = remaining / 3600;
                let mins = (remaining % 3600) / 60;
                let expires_str = format!("{}h {}m", hours, mins);
                let _ = window.global::<Session>().set_trial_expires(expires_str.into());
            }
        }
    } else {
        let _ = window.global::<Session>().set_trial_expires("".into());
    }
}
fn sync_state(window: &MainWindow, state: &AppState, locale: &LocaleManager) {
    sync_all(window, state);
    check_trial_expiry(window, state, locale);
    update_trial_warning(window, state);
}
fn main() -> Result<(), slint::PlatformError> {
    let result = std::thread::Builder::new()
        .name("ui-main".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(ui_main)
        .expect("failed to spawn UI thread")
        .join()
        .expect("UI thread panicked");
    result
}
fn ui_main() -> Result<(), slint::PlatformError> {
    let state = AppState::new();
    let locale = Arc::new(LocaleManager::new());
    let session_cfg = Arc::new(std::sync::Mutex::new(session_config::load()));
    let window = MainWindow::new()?;
    window.global::<Session>().set_dev_mode(cfg!(debug_assertions));
    window.global::<Session>().on_exit_app(|| {
        std::process::exit(0);
    });
    window.global::<Session>().on_open_url(|path: slint::SharedString| {
        let url = format!("{}{}", tbx_entitlements::secrets::server_url(), path.to_string());
        let _ = open::that(url);
    });
    window.global::<Session>().on_dismiss_trial_expired({
        let weak = window.as_weak();
        move || {
            if let Some(w) = weak.upgrade() {
                let _ = w.global::<Session>().set_trial_expired_shown(true);
            }
        }
    });
    window.global::<Session>().on_switch_to_free_plan({
        let weak = window.as_weak();
        let state = state.clone();
        move || {
            if let Some(window) = weak.upgrade() {
                license_net::clear_cache();
                state.gate.clear_token();
                let s = window.global::<Session>();
                let _ = s.set_trial_expired(false);
                let _ = s.set_trial_expired_shown(true);
                sync_all(&window, &state);
                state.bus.publish(tbx_state::AppEvent::PlanChanged);
            }
        }
    });
    let weak_window = window.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(window) = weak_window.upgrade() {
            #[cfg(all(target_os = "windows", target_pointer_width = "64"))]
            {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                let handle_provider = window.window().window_handle();
                if let Ok(handle) = handle_provider.window_handle() {
                    if let RawWindowHandle::Win32(win32) = handle.as_ref() {
                        let hwnd = win32.hwnd.get();
                        if hwnd != 0 {
                            unsafe extern "system" {
                                fn ShowWindow(hwnd: isize, nCmdShow: i32) -> i32;
                                fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
                                fn LoadImageW(hinst: isize, name: *const u16, img_type: u32, cx: i32, cy: i32, flags: u32) -> isize;
                                fn GetModuleHandleW(module_name: *const u16) -> isize;
                            }
                            const SW_MAXIMIZE: i32 = 3;
                            const WM_SETICON: u32 = 0x0080;
                            const ICON_SMALL: usize = 0;
                            const ICON_BIG: usize = 1;
                            const IMAGE_ICON: u32 = 1;
                            const LR_SHARED: u32 = 0x8000;
                            unsafe {
                                ShowWindow(hwnd, SW_MAXIMIZE);
                                let hinst = GetModuleHandleW(std::ptr::null());
                                let icon_id = 1usize as *const u16;
                                let hicon_small = LoadImageW(
                                    hinst, icon_id, IMAGE_ICON,
                                    16, 16, LR_SHARED,
                                );
                                if hicon_small != 0 {
                                    SendMessageW(hwnd, WM_SETICON, ICON_SMALL, hicon_small);
                                }
                                let hicon_big = LoadImageW(
                                    hinst, icon_id, IMAGE_ICON,
                                    32, 32, LR_SHARED,
                                );
                                if hicon_big != 0 {
                                    SendMessageW(hwnd, WM_SETICON, ICON_BIG, hicon_big);
                                }
                            }
                        }
                    }
                }
            }
        }
    });
    let ring_intact = tbx_entitlements::integrity::key_ring_intact();
    let debugger = tbx_entitlements::integrity::debugger_attached();
    if !ring_intact {
        eprintln!("[texelbox] integrity check failed — license restore disabled");
    }
    let hardened = cfg!(not(debug_assertions));
    use crate::SettingsApi;
    if ring_intact && (!hardened || !debugger) {
        if let Some(cache) = license_net::load_cache() {
            if let Ok(token) = tbx_entitlements::token::EntitlementToken::parse(&cache.token_wire) {
                if state.gate.install_cached_token(&token).is_ok() {
                    let settings = window.global::<SettingsApi>();
                    settings.set_license_email(cache.email.into());
                    settings.set_license_key_input(cache.license_key.into());
                    let plan = match state.gate.plan() {
                        tbx_entitlements::Plan::Pro => "Pro",
                        tbx_entitlements::Plan::Trial => "Trial",
                        tbx_entitlements::Plan::Free => "Free",
                    };
                    settings.set_license_status(
                        locale.tr_args("settings-activate-restored", &[("plan", plan.into())]).into()
                    );
                }
            }
        }
    }
    i18n::apply(&window, &locale);
    sync_state(&window, &state, &locale);
    let settings = window.global::<SettingsApi>();
    let plan_label = match state.gate.plan() {
        tbx_entitlements::Plan::Pro => locale.tr("build-plan-pro"),
        tbx_entitlements::Plan::Trial => locale.tr("build-plan-trial"),
        tbx_entitlements::Plan::Free => locale.tr("build-plan-free"),
    };
    settings.set_build_plan(plan_label.into());
    if license_net::load_cache().is_none() {
        let weak = window.as_weak();
        let state_hb = state.clone();
        let locale_trial = locale.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let _ = std::thread::spawn(move || match license_net::activate_trial() {
                Ok(token) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = weak.upgrade() {
                            match state_hb.gate.install_cached_token(&token) {
                                Ok(()) => {
                                    sync_all(&window, &state_hb);
                                    check_trial_expiry(&window, &state_hb, &locale_trial);
                                    update_trial_warning(&window, &state_hb);
                                    state_hb.bus.publish(tbx_state::AppEvent::PlanChanged);
                                }
                                Err(e) => {
                                    if cfg!(debug_assertions) {
                                        eprintln!("[texelbox] trial token install failed: {e:?}");
                                    }
                                    let settings = window.global::<SettingsApi>();
                                    settings.set_license_status(
                                        locale_trial.tr("settings-trial-install-failed").into(),
                                    );
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    if cfg!(debug_assertions) {
                        eprintln!("[texelbox] trial activation failed: {e:?}");
                    }
                }
            });
        });
    }
    settings_panel::wire(&window, state.clone(), locale.clone());
    maps_panel::wire(&window, state.clone(), locale.clone(), session_cfg.clone());
    tileable_panel::wire(&window, state.clone(), locale.clone());
    packing_panel::wire(&window, state.clone(), locale.clone());
    atlas_panel::wire(&window, state.clone(), locale.clone());
    optimize_panel::wire(&window, state.clone(), locale.clone());
    preview_panel::wire(&window, state.clone(), locale.clone());
    batch_panel::wire(&window, state.clone(), locale.clone());
    presets_panel::wire(&window, state.clone(), locale.clone());
    engine_profiles_panel::wire(&window, state.clone(), locale.clone());
    window.global::<Nav>().on_key_switch({
        let weak = window.as_weak();
        move |key: slint::SharedString| {
            let Some(window) = weak.upgrade() else { return };
            let nav = window.global::<Nav>();
            let tab: Option<i32> = match key.as_str() {
                "1" => Some(0),
                "2" => Some(1),
                "3" => Some(2),
                "4" => Some(3),
                "5" => Some(4),
                "6" => Some(5),
                "7" => Some(6),
                "8" => Some(7),
                "9" => Some(8),
                "0" => Some(9),
                _ => None,
            };
            if let Some(t) = tab {
                nav.set_tab(t);
                nav.invoke_tab_selected(t);
            }
        }
    });
    {
        let weak = window.as_weak().clone();
        let state_hb = state.clone();
        let locale_hb = locale.clone();
        std::thread::spawn(move || {
            loop {
                let outcome = license_net::heartbeat();
                match outcome {
                    license_net::HeartbeatOutcome::Refreshed(token) => {
                        let _ = slint::invoke_from_event_loop({
                            let weak = weak.clone();
                            let state = state_hb.clone();
                            let locale = locale_hb.clone();
                            move || {
                                let _ = state.gate.install_token(&token);
                                if let Some(window) = weak.upgrade() {
                                    sync_state(&window, &state, &locale);
                                    state.bus.publish(tbx_state::AppEvent::PlanChanged);
                                }
                            }
                        });
                    }
                    license_net::HeartbeatOutcome::Revoked => {
                        let _ = slint::invoke_from_event_loop({
                            let weak = weak.clone();
                            let state = state_hb.clone();
                            let locale = locale_hb.clone();
                            move || {
                                state.gate.clear_token();
                                if let Some(window) = weak.upgrade() {
                                    let settings = window.global::<SettingsApi>();
                                    settings.set_license_status(
                                        locale.tr("settings-heartbeat-revoked").into(),
                                    );
                                    sync_state(&window, &state, &locale);
                                    state.bus.publish(tbx_state::AppEvent::PlanChanged);
                                }
                            }
                        });
                    }
                    license_net::HeartbeatOutcome::NoLicense => {}
                    license_net::HeartbeatOutcome::Error(msg) => {
                        if cfg!(debug_assertions) {
                            eprintln!("[texelbox] heartbeat failed: {msg}");
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(
                    license_net::HEARTBEAT_INTERVAL_SECS,
                ));
            }
        });
    }
    {
        let weak = window.as_weak().clone();
        let locale_uv = locale.clone();
        std::thread::spawn(move || {
            if cfg!(debug_assertions) {
                eprintln!(
                    "[texelbox] update: checking {}/app/version (running v{})",
                    tbx_entitlements::secrets::server_url(),
                    update_net::APP_VERSION
                );
            }
            match update_net::check_latest() {
                Err(e) => {
                    if cfg!(debug_assertions) {
                        eprintln!("[texelbox] update: check failed (offline?) — {e}");
                    }
                }
                Ok(release) => {
                    if update_net::is_newer(&release.version, update_net::APP_VERSION) {
                        if cfg!(debug_assertions) {
                            eprintln!(
                                "[texelbox] update: newer version {} available → {}",
                                release.version, release.url
                            );
                        }
                        if release.url.trim().is_empty() {
                            return;
                        }
                        update_net::remember_download_url(release.url.clone());
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(window) = weak.upgrade() else { return };
                            let session = window.global::<Session>();
                            session.set_update_available(true);
                            session.set_update_note(
                                locale_uv
                                    .tr_args(
                                        "update-available-note",
                                        &[("version", release.version.as_str().into())],
                                    )
                                    .into(),
                            );
                        });
                    } else if cfg!(debug_assertions) {
                        eprintln!(
                            "[texelbox] update: v{} is current (latest is {})",
                            update_net::APP_VERSION,
                            release.version
                        );
                    }
                }
            }
        });
    }
    if cfg!(debug_assertions) {
        if let Some(ms) = std::env::var("TEXELBOX_SMOKE_EXIT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            let timer = Box::leak(Box::new(slint::Timer::default()));
            timer.start(
                slint::TimerMode::SingleShot,
                std::time::Duration::from_millis(ms),
                || {
                    let _ = slint::quit_event_loop();
                },
            );
        }
    }
    let result = window.run();
    if cfg!(debug_assertions) {
        let size = window.window().size();
        eprintln!(
            "[texelbox] window closed cleanly · maximized={} · final size {}×{}",
            window.window().is_maximized(),
            size.width,
            size.height
        );
    }
    result
}
