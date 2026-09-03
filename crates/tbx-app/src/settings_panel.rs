use std::sync::Arc;
use slint::ComponentHandle;
use tbx_entitlements::Plan;
use tbx_locale::LocaleManager;
use tbx_state::{AppEvent, AppState};
use crate::{i18n, license_net, MainWindow, Session, SettingsApi};
const LANG_TAGS: &[&str] = &["en-US", "es-ES"];
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let api = window.global::<SettingsApi>();
    api.set_update_check_status(locale.tr("update-check-status-idle").into());
    if let Some(idx) = LANG_TAGS.iter().position(|t| *t == locale.language()) {
        api.set_language_index(idx as i32);
    }
    api.on_language_changed({
        let state = state.clone();
        let locale = locale.clone();
        let proxy = window.as_weak().clone();
        move |index: i32| {
            let Some(tag) = LANG_TAGS.get(index as usize) else { return };
            let Some(window) = proxy.upgrade() else { return };
            if locale.set_language(tag) {
                i18n::apply(&window, &locale);
                crate::sync_all(&window, &state);
                state.bus.publish(AppEvent::LanguageChanged);
            }
        }
    });
    api.on_dev_toggle_license({
        let state = state.clone();
        let proxy = window.as_weak().clone();
        move || {
            let Some(window) = proxy.upgrade() else { return };
            if !cfg!(debug_assertions) {
                return;
            }
            let target = if state.gate.plan() == Plan::Pro { Plan::Free } else { Plan::Pro };
            if let Ok(token) = tbx_entitlements::EntitlementGate::mint_dev_token(target, 30 * 24 * 3600) {
                let _ = state.gate.install_token(&token);
            }
            crate::sync_all(&window, &state);
            state.bus.publish(AppEvent::PlanChanged);
        }
    });
    api.on_activate_license({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<SettingsApi>();
            if api.get_activating() {
                return;
            }
            let email = api.get_license_email().to_string();
            let password = api.get_license_password().to_string();
            let key = api.get_license_key_input().to_string();
            if email.trim().is_empty() || password.is_empty() || key.trim().is_empty() {
                api.set_license_status(locale.tr("settings-enter-details").into());
                return;
            }
            api.set_activating(true);
            api.set_license_status(locale.tr("settings-activating").into());
            let weak_cb = weak.clone();
            let state_cb = state.clone();
            let locale_cb = locale.clone();
            std::thread::spawn(move || {
                let outcome = license_net::activate(&email, &password, &key);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak_cb.upgrade() else { return };
                    let api = window.global::<SettingsApi>();
                    api.set_activating(false);
                    match outcome {
                        Ok(token) => {
                                match state_cb.gate.install_token(&token) {
                                    Ok(()) => {
                                        let plan = match state_cb.gate.plan() {
                                            tbx_entitlements::Plan::Pro => "Pro",
                                            tbx_entitlements::Plan::Trial => "Trial",
                                            tbx_entitlements::Plan::Free => "Free",
                                        };
                                        api.set_license_status(
                                            locale_cb
                                                .tr_args("settings-activate-ok", &[("plan", plan.into())])
                                                .into(),
                                        );
                                        crate::sync_all(&window, &state_cb);
                                        state_cb.bus.publish(AppEvent::PlanChanged);
                                    }
                                Err(e) => {
                                    api.set_license_status(
                                        locale_cb
                                            .tr_args(
                                                "settings-activate-fail",
                                                &[("error", e.to_string().as_str().into())],
                                            )
                                            .into(),
                                    );
                                }
                            }
                        }
                        Err(msg) => {
                            api.set_license_status(
                                locale_cb
                                    .tr_args("settings-activate-fail", &[("error", msg.as_str().into())])
                                    .into(),
                            );
                        }
                    }
                });
            });
        }
    });
    api.on_deactivate_license({
        let weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let api = window.global::<SettingsApi>();
            license_net::clear_cache();
            state.gate.clear_token();
            crate::sync_all(&window, &state);
            state.bus.publish(AppEvent::PlanChanged);
            api.set_license_status(locale.tr("settings-activate-deactivated").into());
        }
    });
    api.on_open_update_page(|| {
        let Some(url) = crate::update_net::download_url() else { return };
        if url.is_empty() {
            return;
        }
        let _ = std::process::Command::new("cmd")
            .arg("/C")
            .arg(format!("start \"\" \"{}\"", url.replace('"', "")))
            .spawn();
    });
    api.on_check_for_updates({
        let proxy = window.as_weak().clone();
        let locale_cb = locale.clone();
        move || {
            let Some(window) = proxy.upgrade() else { return };
            let api = window.global::<SettingsApi>();
            if api.get_checking_updates() {
                return;
            }
            api.set_checking_updates(true);
            api.set_update_check_status(
                locale_cb.tr("update-check-status-checking").into(),
            );
            let weak = window.as_weak().clone();
            let locale = locale_cb.clone();
            std::thread::spawn(move || {
                let result = crate::update_net::check_latest();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak.upgrade() else { return };
                    let api = window.global::<SettingsApi>();
                    let session = window.global::<Session>();
                    match result {
                        Ok(release) => {
                            if crate::update_net::is_newer(&release.version, crate::update_net::APP_VERSION) {
                                crate::update_net::remember_download_url(release.url.clone());
                                session.set_update_available(true);
                                session.set_update_note(
                                    locale
                                        .tr_args(
                                            "update-available-note",
                                            &[("version", release.version.as_str().into())],
                                        )
                                        .into(),
                                );
                                api.set_update_check_status(
                                    locale
                                        .tr_args(
                                            "update-check-status-available",
                                            &[("version", release.version.as_str().into())],
                                        )
                                        .into(),
                                );
                            } else {
                                session.set_update_available(false);
                                session.set_update_note("".into());
                                api.set_update_check_status(
                                    locale.tr("update-check-status-current").into(),
                                );
                            }
                        }
                        Err(e) => {
                            if cfg!(debug_assertions) {
                                eprintln!("[texelbox] update: manual check failed — {e}");
                            }
                            api.set_update_check_status(
                                locale.tr("update-check-status-offline").into(),
                            );
                        }
                    }
                    api.set_checking_updates(false);
                });
            });
        }
    });
    api.on_open_server_url(|path: slint::SharedString| {
        let url = format!("{}{}", tbx_entitlements::secrets::server_url(), path.to_string());
        let _ = open::that(url);
    });
    api.on_buy_pro({
        let proxy = window.as_weak().clone();
        move || {
            let Some(window) = proxy.upgrade() else { return };
            let api = window.global::<SettingsApi>();
            api.invoke_open_server_url("/pricing".into());
        }
    });
    api.on_exit_app(|| {
        std::process::exit(0);
    });
}
