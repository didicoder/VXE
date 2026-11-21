// DEFAULTS 的 json! 宏嵌套较深，默认递归上限不够。
#![recursion_limit = "256"]

use tauri::{Manager, WebviewWindowBuilder};

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
    // WebView2 默认用户数据目录也钉到同一处：任何一个窗口漏设
    // `data_directory` 时，都不会在别处重新长出缓存。
    if let Some(dir) = window_mgr::webview_data_dir() {
        // 安全：进程刚启动、还没有其他线程使用环境变量。
        unsafe {
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &dir);
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 主窗手动创建（tauri.conf 里 create:false），并固定 WebView2 用户数据目录，
            // 让缓存与便携 exe 同级。
            if let Some(conf) = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == window_mgr::MAIN)
            {
                let mut builder = WebviewWindowBuilder::from_config(app.handle(), conf)?;
                if let Some(dir) = window_mgr::webview_data_dir() {
                    builder = builder.data_directory(dir);
                }
                builder.build()?;
            }
            let mut store = config::Store::load();
            let _ = store.migrate_scales();
            let _ = store.split_surfaces();
            let _ = store.cleanup_dead_keys();
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
            window_mgr::prewarm(app.handle());
            window_mgr::apply_main_geometry(app.handle());
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
