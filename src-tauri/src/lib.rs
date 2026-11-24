// DEFAULTS 的 json! 宏嵌套较深，默认递归上限不够。
#![recursion_limit = "256"]

use std::ffi::OsStr;

use tauri::{Listener, Manager};

const WEBVIEW2_DEFAULT_BROWSER_ARGUMENTS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

pub(crate) fn browser_arguments(existing: &OsStr) -> String {
    let existing = existing.to_string_lossy();
    let has_disable_breakpad = existing
        .split_whitespace()
        .any(|argument| argument == "--disable-breakpad");
    let has_disable_features = existing
        .split_whitespace()
        .any(|argument| argument.starts_with("--disable-features="));
    let mut arguments = String::new();
    if !has_disable_features {
        arguments.push_str(WEBVIEW2_DEFAULT_BROWSER_ARGUMENTS);
    }
    if !existing.trim().is_empty() {
        if !arguments.is_empty() {
            arguments.push(' ');
        }
        arguments.push_str(existing.trim());
    }
    if !has_disable_breakpad {
        if !arguments.is_empty() {
            arguments.push(' ');
        }
        arguments.push_str("--disable-breakpad");
    }
    arguments
}

mod autostart;
mod battery;
mod commands;
mod config;
mod discover;
mod hover;
mod refresh;
mod sound;
mod theme;
mod tray;
mod tray_icon;
mod window_mgr;
mod winutil;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let existing = std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
    // SAFETY: no worker threads or WebView environments exist before application setup.
    unsafe {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            browser_arguments(&existing),
        );
    }
    // WebView2 默认用户数据目录也钉到同一处：任何一个窗口漏设
    // `data_directory` 时，都不会在别处重新长出缓存。
    if let Some(dir) = window_mgr::webview_data_dir() {
        // 安全：进程刚启动、还没有其他线程使用环境变量。
        unsafe {
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &dir);
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !args.iter().any(|arg| arg == "--tray") {
                window_mgr::show_main(app, "");
            }
        }))
        .manage(window_mgr::Runtime::default())
        .manage(refresh::Scheduler::default())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 托盘常驻时不创建任何 WebView；界面按需创建，订阅就绪后才显示。
            let mut store = config::Store::load();
            let _ = store.migrate_scales();
            let _ = store.split_surfaces();
            let _ = store.cleanup_dead_keys();
            let _ = store.migrate_device_labels();
            let hotkey_enabled = store
                .get("hotkey_enabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let hotkey = store
                .get("hotkey")
                .and_then(|value| value.as_str())
                .unwrap_or("ctrl+alt+d")
                .to_string();
            #[cfg(not(debug_assertions))]
            let autostart = store
                .get("autostart")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            app.manage(commands::AppState {
                store: std::sync::Mutex::new(store),
                passive: crate::battery::passive::Hub::new(),
                statuses: std::sync::Mutex::new(Vec::new()),
                refresh_lock: std::sync::Mutex::new(()),
                low_notify: std::sync::Mutex::new(std::collections::HashMap::new()),
                background_draft: std::sync::Mutex::new(None),
                alerts: std::sync::Mutex::new(std::collections::VecDeque::new()),
                alert_showing: std::sync::atomic::AtomicBool::new(false),
                current_alert: std::sync::Mutex::new(None),
                tooltip_hold: std::sync::atomic::AtomicBool::new(false),
                tooltip_closing: std::sync::atomic::AtomicBool::new(false),
                popup_hold: std::sync::atomic::AtomicBool::new(false),
                alert_preview: std::sync::atomic::AtomicBool::new(false),
                last_tray_click: std::sync::Mutex::new(None),
                registered_hotkey: std::sync::Mutex::new(None),
                inspector_target: std::sync::Mutex::new(None),
                inspector_placed: std::sync::atomic::AtomicBool::new(false),
                bg_editor_placed: std::sync::atomic::AtomicBool::new(false),
            });
            tray::create(app.handle())?;
            #[cfg(not(debug_assertions))]
            if let Ok(exe) = std::env::current_exe() {
                let _ = autostart::migrate(autostart, &exe);
            }
            window_mgr::start_reaper(app.handle().clone());
            let refresh_app = app.handle().clone();
            app.listen("refresh-request", move |_| {
                refresh::request_manual(refresh_app.clone());
            });
            theme::apply(app.handle());
            theme::apply_icon_for_current(app.handle());
            theme::start(app.handle().clone());
            refresh::start(app.handle().clone());
            hover::start(app.handle().clone());
            if hotkey_enabled {
                let _ = commands::register_hotkey(app.handle().clone(), hotkey);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::window_ready,
            commands::remember_main_page,
            commands::get_main_page,
            commands::get_tray_overlay_mode,
            commands::get_config,
            commands::set_setting,
            commands::set_settings,
            commands::get_devices,
            commands::refresh_now,
            commands::add_device,
            commands::update_device,
            commands::reorder_devices,
            commands::remove_device,
            commands::scan_devices,
            commands::get_profile_choices,
            commands::get_model_choices,
            commands::detect_device,
            commands::add_discovered_device,
            commands::get_sound_files,
            commands::open_config_dir,
            commands::quit_app,
            commands::show_main,
            commands::show_add,
            commands::show_card_inspector,
            commands::hide_card_inspector,
            commands::get_card_inspector_target,
            commands::preview_card,
            commands::show_settings,
            commands::get_config_path,
            commands::get_background_state,
            commands::preview_background,
            commands::commit_background,
            commands::cancel_background_preview,
            commands::show_background_editor,
            commands::hide_background_editor,
            commands::set_device_card_image,
            commands::set_device_card_image_from_path,
            commands::pick_card_image_file,
            commands::get_device_card_image,
            commands::clear_device_card_image,
            commands::test_tray_blink,
            commands::list_unknown_hid,
            commands::probe_unknown_hid,
            commands::reset_settings,
            commands::test_alert,
            commands::get_current_alert,
            commands::preview_popup,
            commands::preview_tooltip,
            commands::preview_alert,
            commands::preview_sound,
            commands::start_alert_position,
            commands::save_alert_position,
            commands::reset_alert_position,
            commands::register_hotkey,
            commands::unregister_hotkey,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(&event, tauri::RunEvent::MainEventsCleared) {
                window_mgr::drain_actions(app);
            }
            match &event {
                // 回收最后一个 WebView 后继续托盘常驻；菜单显式 exit(0) 仍正常退出。
                tauri::RunEvent::ExitRequested {
                    code: None, api, ..
                } => api.prevent_exit(),
                tauri::RunEvent::ExitRequested { code: Some(_), .. } | tauri::RunEvent::Exit => {
                    if let Some(state) = app.try_state::<commands::AppState>() {
                        state.passive.shutdown();
                    }
                }
                _ => {}
            }
        });
}
