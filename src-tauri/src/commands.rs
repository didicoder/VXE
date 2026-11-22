use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::battery::{DeviceSpec, DeviceStatus};
use crate::config::{self, Store};
use crate::discover;
use crate::refresh::{self, AlertPayload, LowNotifyState};
use crate::window_mgr;

pub struct AppState {
    pub store: Mutex<Store>,
    pub statuses: Mutex<Vec<DeviceStatus>>,
    pub refresh_lock: Mutex<()>,
    pub low_notify: Mutex<HashMap<String, LowNotifyState>>,
    pub background_draft: Mutex<Option<BackgroundState>>,
    pub alerts: Mutex<VecDeque<AlertPayload>>,
    pub alert_showing: AtomicBool,
    pub current_alert: Mutex<Option<AlertPayload>>,
    pub tooltip_hold: AtomicBool,
    pub tooltip_closing: AtomicBool,
    pub popup_hold: AtomicBool,
    pub alert_preview: AtomicBool,
    pub last_tray_click: Mutex<Option<std::time::Instant>>,
    pub registered_hotkey: Mutex<Option<String>>,
    pub inspector_target: Mutex<Option<String>>,
    pub inspector_placed: AtomicBool,
    pub bg_editor_placed: AtomicBool,
}

#[derive(Debug, Serialize)]
pub struct ConfigSnapshot {
    settings: Map<String, Value>,
    /// 设备**原样透传**：不能过 `DeviceSpec`，它只认硬件字段（id/vid/pid/icon…），
    /// `card_*` 等扩展配置会被序列化丢掉——表现为卡片缩放/偏移等设置存得进盘、
    /// 但每次 `load()` 后界面都还原，用户以为“设置无效”。
    devices: Vec<Value>,
}

/// 背景可调参数 + 当前图片（编辑器预览草稿与保存共用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundState {
    pub image: Option<String>,
    pub ratio: f64,
    pub dx: f64,
    pub dy: f64,
    pub blur: f64,
    pub dim: f64,
}

#[tauri::command]
pub fn window_ready(window: tauri::WebviewWindow) -> Result<(), String> {
    window_mgr::on_ready(&window)
}

#[tauri::command]
pub fn get_main_page(app: AppHandle) -> String {
    window_mgr::main_page(&app)
}

#[tauri::command]
pub fn remember_main_page(app: AppHandle, page: String) {
    if matches!(page.as_str(), "summary" | "devices" | "settings") {
        window_mgr::remember_main_page(&app, &page);
    }
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<ConfigSnapshot, String> {
    let store = state.store.lock().map_err(|_| "config lock poisoned")?;
    Ok(config_snapshot(store.data(), store.devices()))
}

fn config_snapshot(data: &Map<String, Value>, devices: &[Value]) -> ConfigSnapshot {
    let mut settings = data.clone();
    settings.remove("devices");
    ConfigSnapshot {
        settings,
        devices: devices.to_vec(),
    }
}

fn popup_layout_key(key: &str) -> bool {
    matches!(key, "popup_base" | "popup_scale" | "display_popup")
}

fn tooltip_layout_key(key: &str) -> bool {
    matches!(key, "tip_base" | "tip_scale" | "display_tip")
}

#[tauri::command]
pub fn set_setting(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: Value,
) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
    store
        .set(&key, value.clone())
        .map_err(|err| err.to_string())?;
    drop(store);
    match key.as_str() {
        "autostart" => sync_autostart(value.as_bool().unwrap_or(false)),
        "refresh_seconds" => refresh::reschedule(&app),
        "ui_base" | "ui_scale" => window_mgr::apply_main_geometry(&app),
        key if popup_layout_key(key) => window_mgr::relayout_popup(&app),
        key if tooltip_layout_key(key) => window_mgr::relayout_tooltip(&app),
        "alert_base" | "alert_scale" => relayout_alert_preview(&app),
        "hotkey" | "hotkey_enabled" => reload_hotkey(&app),
        "theme_mode" | "theme_light_at" | "theme_dark_at" => crate::theme::apply(&app),
        "ui_theme" => crate::theme::apply_icon_for_current(&app),
        "good_level" | "warn_level" | "tray_level_mode" | "tray_level_device" => {
            crate::tray::refresh_battery_icon(&app)
        }
        _ => {}
    }
    let _ = app.emit("config-changed", ());
    Ok(())
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: Map<String, Value>,
) -> Result<(), String> {
    let autostart = patch.get("autostart").and_then(|value| value.as_bool());
    let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
    let touch_refresh = patch.contains_key("refresh_seconds");
    let touch_scale = patch.contains_key("ui_base") || patch.contains_key("ui_scale");
    let touch_popup = patch.keys().any(|key| popup_layout_key(key));
    let touch_tip = patch.keys().any(|key| tooltip_layout_key(key));
    let touch_alert = patch.contains_key("alert_base") || patch.contains_key("alert_scale");
    let touch_hotkey = patch.contains_key("hotkey") || patch.contains_key("hotkey_enabled");
    let touch_theme = patch.contains_key("theme_mode")
        || patch.contains_key("theme_light_at")
        || patch.contains_key("theme_dark_at");
    let touch_tray = patch.contains_key("good_level")
        || patch.contains_key("warn_level")
        || patch.contains_key("tray_level_mode")
        || patch.contains_key("tray_level_device");
    for (key, value) in patch {
        store.set(&key, value).map_err(|err| err.to_string())?;
    }
    drop(store);
    if let Some(enabled) = autostart {
        sync_autostart(enabled);
    }
    if touch_refresh {
        refresh::reschedule(&app);
    }
    if touch_scale {
        window_mgr::apply_main_geometry(&app);
    }
    if touch_popup {
        window_mgr::relayout_popup(&app);
    }
    if touch_tip {
        window_mgr::relayout_tooltip(&app);
    }
    if touch_alert {
        relayout_alert_preview(&app);
    }
    if touch_hotkey {
        reload_hotkey(&app);
    }
    if touch_theme {
        crate::theme::apply(&app);
    }
    if touch_tray {
        crate::tray::refresh_battery_icon(&app);
    }
    let _ = app.emit("config-changed", ());
    Ok(())
}

#[tauri::command]
pub fn add_device(
    app: AppHandle,
    state: State<'_, AppState>,
    device: Value,
) -> Result<Value, String> {
    let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
    let added = store.add_device(device).map_err(|err| err.to_string())?;
    // 手动添加解除墓碑，允许用户把删过的设备加回来
    let _ = store.unmark_removed(&config::hardware_key_of(&added));
    drop(store);
    let _ = app.emit("config-changed", ());
    Ok(added)
}

#[tauri::command]
pub fn update_device(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    fields: Map<String, Value>,
) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
    store
        .update_device(&id, fields)
        .map_err(|err| err.to_string())?;
    drop(store);
    let _ = app.emit("config-changed", ());
    Ok(())
}

#[tauri::command]
pub fn reorder_devices(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
    store.reorder_devices(&ids).map_err(|err| err.to_string())?;
    drop(store);
    let _ = app.emit("config-changed", ());
    Ok(())
}

#[tauri::command]
pub fn remove_device(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
    store.remove_device(&id).map_err(|err| err.to_string())?;
    drop(store);
    let _ = app.emit("config-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn scan_devices(
    state: State<'_, AppState>,
) -> Result<Vec<discover::DiscoveredDevice>, String> {
    let known: Vec<DeviceSpec> = {
        let store = state.store.lock().map_err(|_| "config lock poisoned")?;
        store.devices().iter().map(DeviceSpec::from_json).collect()
    };
    tauri::async_runtime::spawn_blocking(move || {
        let dirs = refresh::profile_dirs();
        discover::scan(&dirs, &known)
    })
    .await
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn get_profile_choices() -> Vec<discover::ProfileChoice> {
    let dirs = refresh::profile_dirs();
    tauri::async_runtime::spawn_blocking(move || discover::profile_choices(&dirs))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn get_model_choices() -> Vec<crate::battery::models::ModelChoice> {
    let dirs = refresh::profile_dirs();
    tauri::async_runtime::spawn_blocking(move || discover::model_choices(&dirs))
        .await
        .unwrap_or_default()
}

/// B：列出未命中协议的厂商 HID 接口（智能探测对象）。
#[tauri::command]
pub async fn list_unknown_hid() -> Vec<discover::UnknownHid> {
    let dirs = refresh::profile_dirs();
    tauri::async_runtime::spawn_blocking(move || discover::scan_unknown(&dirs))
        .await
        .unwrap_or_default()
}

/// B：对未知接口轮试协议模板；命中即生成协议 JSON 落盘。
#[tauri::command]
pub async fn probe_unknown_hid(
    unknown: discover::UnknownHid,
) -> Result<discover::ProbeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        discover::probe_unknown(&refresh::profile_dirs(), &unknown)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn detect_device(
    device: DeviceSpec,
    profile_id: Option<String>,
) -> Result<discover::DetectResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        discover::detect(
            &device,
            profile_id.as_deref().unwrap_or_default(),
            &refresh::profile_dirs(),
        )
    })
    .await
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn add_discovered_device(
    app: AppHandle,
    state: State<'_, AppState>,
    device: discover::DiscoveredDevice,
) -> Result<Value, String> {
    let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
    let added = store
        .add_device(discover::to_device_value(&device))
        .map_err(|err| err.to_string())?;
    // 手动添加解除墓碑，允许用户把删过的设备加回来
    let _ = store.unmark_removed(&config::hardware_key_of(&added));
    drop(store);
    let _ = app.emit("config-changed", ());
    Ok(added)
}

#[tauri::command]
pub fn get_devices(state: State<'_, AppState>) -> Vec<DeviceStatus> {
    state
        .statuses
        .lock()
        .map(|statuses| statuses.clone())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn refresh_now(app: AppHandle) -> Result<Vec<DeviceStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || refresh::manual_refresh(&app))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn get_sound_files(app: AppHandle) -> Vec<String> {
    let resource_dir = app.path().resource_dir().ok();
    let dirs = crate::sound::search_dirs(
        Some(&config::program_dir()),
        resource_dir.as_deref(),
        Some(&config::program_dir()),
    );
    let mut names = Vec::new();
    for dir in dirs {
        for wav in crate::sound::list_wavs(&dir) {
            let Some(name) = wav.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if names.iter().any(|existing| existing == name) {
                continue;
            }
            names.push(name.to_string());
        }
    }
    // 内置提示音（随二进制打包）也列出，外部同名文件已覆盖时去重
    for (name, _) in crate::sound::EMBEDDED {
        if !names.iter().any(|existing| existing == name) {
            names.push(name.to_string());
        }
    }
    names
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertTestPayload {
    pub device: Option<String>,
    pub battery: Option<u8>,
}

#[tauri::command]
pub fn get_current_alert(state: State<'_, AppState>) -> Option<AlertPayload> {
    state
        .current_alert
        .lock()
        .ok()
        .and_then(|alert| alert.clone())
}

#[tauri::command]
pub fn test_alert(app: AppHandle, payload: Option<AlertTestPayload>) {
    let payload = payload.unwrap_or(AlertTestPayload {
        device: Some("设备".to_string()),
        battery: Some(20),
    });
    let alert = AlertPayload {
        device: payload.device.unwrap_or_else(|| "设备".to_string()),
        battery: payload.battery.unwrap_or(20),
        color: None,
    };
    let (play_sound, sound_file) = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .store
                .lock()
                .ok()
                .map(|store| (store.alert_sound(), store.alert_sound_file()))
        })
        .unwrap_or((false, String::new()));
    refresh::enqueue_alert(&app, alert, play_sound, &sound_file);
}

#[tauri::command]
pub fn preview_popup(app: AppHandle, mode: Option<String>) {
    let mode = mode.unwrap_or_else(|| "show".to_string());
    match mode.as_str() {
        "hide" => {
            if let Some(state) = app.try_state::<AppState>() {
                state
                    .popup_hold
                    .store(false, std::sync::atomic::Ordering::SeqCst);
            }
            window_mgr::hide_popup(&app);
        }
        "hold" => {
            window_mgr::show_popup(&app, true);
        }
        _ => {
            window_mgr::show_popup(&app, false);
        }
    }
    let _ = app.emit("preview-popup", mode);
}

#[tauri::command]
pub fn preview_tooltip(app: AppHandle, mode: Option<String>) {
    let mode = mode.unwrap_or_else(|| "show".to_string());
    if mode == "hide" {
        window_mgr::hide_tooltip_preview(&app);
    } else {
        window_mgr::show_tooltip(&app, mode == "hold");
    }
    let _ = app.emit("preview-tooltip", mode);
}

fn relayout_alert_preview(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if !state
        .alert_preview
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return;
    }
    let visible = app
        .get_webview_window(window_mgr::ALERT)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    if !visible {
        return;
    }
    window_mgr::show_alert_event(app, "alert-preview", Value::Null);
}

#[tauri::command]
pub fn preview_alert(app: AppHandle, mode: Option<String>) {
    let mode = mode.unwrap_or_else(|| "show".to_string());
    let hold = mode == "hold";
    if let Some(state) = app.try_state::<AppState>() {
        state
            .alert_preview
            .store(hold, std::sync::atomic::Ordering::SeqCst);
    }
    if mode == "hide" {
        window_mgr::hide(&app, window_mgr::ALERT);
        return;
    }
    let payload = crate::refresh::sample_alert(&app);
    window_mgr::show_alert_event(&app, "alert-preview", serde_json::json!(payload));
}

#[tauri::command]
pub fn preview_sound(app: AppHandle, file: String) {
    crate::refresh::play_sound_file(&app, &file);
}

#[tauri::command]
pub fn start_alert_position(app: AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state
            .alert_preview
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
    window_mgr::show_alert_event(&app, "alert-position-start", Value::Null);
}

#[tauri::command]
pub fn save_alert_position(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let Some(window) = app.get_webview_window("alert") else {
        return Ok(());
    };
    let position = window.outer_position().map_err(|err| err.to_string())?;
    let size = window.outer_size().map_err(|err| err.to_string())?;
    let center_x = position.x + size.width as i32 / 2;
    let center_y = position.y + size.height as i32 / 2;
    {
        let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
        store
            .set("alert_pos", serde_json::json!([center_x, center_y]))
            .map_err(|err| err.to_string())?;
    }
    state
        .alert_preview
        .store(false, std::sync::atomic::Ordering::SeqCst);
    window_mgr::set_alert_position_mode(&app, false);
    let _ = window.hide();
    let _ = app.emit("alert-position-save", ());
    Ok(())
}

#[tauri::command]
pub fn reset_alert_position(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
        store
            .set("alert_pos", Value::Null)
            .map_err(|err| err.to_string())?;
    }
    state
        .alert_preview
        .store(false, std::sync::atomic::Ordering::SeqCst);
    window_mgr::set_alert_position_mode(&app, false);
    window_mgr::hide(&app, "alert");
    let _ = app.emit("alert-position-reset", ());
    Ok(())
}

#[tauri::command]
pub fn register_hotkey(app: AppHandle, accelerator: String) -> bool {
    let Ok(shortcut) = accelerator.parse::<tauri_plugin_global_shortcut::Shortcut>() else {
        return false;
    };
    let state = app.try_state::<AppState>();
    let old = state.as_ref().and_then(|state| {
        state
            .registered_hotkey
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
    });
    if old.as_deref() == Some(accelerator.as_str()) {
        return true;
    }
    if app
        .global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                window_mgr::show_main(app, "");
            }
        })
        .is_err()
    {
        // Keep the previously working accelerator registered.
        return false;
    }
    if let Some(old) = old
        && let Ok(old_shortcut) = old.parse::<tauri_plugin_global_shortcut::Shortcut>()
    {
        let _ = app.global_shortcut().unregister(old_shortcut);
    }
    if let Some(state) = state
        && let Ok(mut slot) = state.registered_hotkey.lock()
    {
        *slot = Some(accelerator);
    }
    true
}

#[tauri::command]
pub fn unregister_hotkey(app: AppHandle) {
    let _ = app.global_shortcut().unregister_all();
    if let Some(state) = app.try_state::<AppState>()
        && let Ok(mut slot) = state.registered_hotkey.lock()
    {
        *slot = None;
    }
}

#[tauri::command]
pub fn open_config_dir() -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(config::app_dir())
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn show_main(app: AppHandle, page: Option<String>) {
    window_mgr::show_main(&app, page.as_deref().unwrap_or("summary"));
}

#[tauri::command]
pub fn get_tray_overlay_mode(app: AppHandle) -> String {
    window_mgr::tray_overlay_mode(&app).to_owned()
}

#[tauri::command]
pub fn show_add(app: AppHandle) -> Result<(), String> {
    window_mgr::show_add(&app)
}

#[tauri::command]
pub fn show_card_inspector(
    app: AppHandle,
    state: State<'_, AppState>,
    device_id: String,
) -> Result<(), String> {
    if let Ok(mut slot) = state.inspector_target.lock() {
        *slot = Some(device_id.clone());
    }
    // The lifecycle manager emits inspector-open after the window is ready.
    window_mgr::show_inspector(&app)
}

#[tauri::command]
pub fn hide_card_inspector(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut slot) = state.inspector_target.lock() {
        *slot = None;
    }
    window_mgr::hide_inspector(&app);
    Ok(())
}

#[tauri::command]
pub fn get_card_inspector_target(state: State<'_, AppState>) -> Option<String> {
    state
        .inspector_target
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

/// 卡片设置窗的实时预览只发给主窗，不落盘；空 patch 表示清除预览。
#[tauri::command]
pub fn preview_card(app: AppHandle, id: String, patch: Value) -> Result<(), String> {
    let _ = app.emit_to(
        window_mgr::MAIN,
        "card-preview",
        serde_json::json!({ "id": id, "patch": patch }),
    );
    Ok(())
}

#[tauri::command]
pub fn get_config_path() -> String {
    config::config_path().display().to_string()
}

/// 自定义窗口壁纸文件：配置目录下的 `background.dat`（整段 data URL，不进 config.json）。
fn background_path() -> std::path::PathBuf {
    config::app_dir().join("background.dat")
}

fn read_background_image() -> Result<Option<String>, String> {
    match std::fs::read_to_string(background_path()) {
        Ok(text) => {
            let trimmed = text.trim();
            Ok(trimmed
                .starts_with("data:image/")
                .then(|| trimmed.to_string()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

fn clamp_bg(value: f64, lo: f64, hi: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        fallback
    }
}

fn normalize_background_state(mut state: BackgroundState) -> Result<BackgroundState, String> {
    if let Some(data) = state.image.take() {
        let trimmed = data.trim().to_string();
        if !trimmed.starts_with("data:image/") {
            return Err("背景图必须是 data URL 图片".into());
        }
        if trimmed.len() > 12 * 1024 * 1024 {
            return Err("背景图过大（原图请不超过 8 MB）".into());
        }
        state.image = Some(trimmed);
    }
    state.ratio = clamp_bg(state.ratio, 1.0, 3.0, 1.0);
    state.dx = clamp_bg(state.dx, -50.0, 50.0, 0.0);
    state.dy = clamp_bg(state.dy, -50.0, 50.0, 0.0);
    state.blur = clamp_bg(state.blur, 0.0, 24.0, 12.0);
    state.dim = clamp_bg(state.dim, 0.0, 60.0, 0.0);
    Ok(state)
}

/// 当前生效的背景状态：预览草稿优先，否则读保存的图片与配置。
#[tauri::command]
pub fn get_background_state(state: State<'_, AppState>) -> Result<BackgroundState, String> {
    if let Ok(draft) = state.background_draft.lock()
        && let Some(draft) = draft.as_ref()
    {
        return Ok(draft.clone());
    }
    let store = state.store.lock().map_err(|_| "config lock poisoned")?;
    Ok(BackgroundState {
        image: read_background_image()?,
        ratio: store.bg_ratio(),
        dx: store.bg_dx(),
        dy: store.bg_dy(),
        blur: store.bg_blur(),
        dim: store.bg_dim(),
    })
}

/// 预览：只进内存草稿并广播，编辑器保持打开；不落盘。
#[tauri::command]
pub fn preview_background(
    app: AppHandle,
    state: State<'_, AppState>,
    background: BackgroundState,
) -> Result<(), String> {
    let normalized = normalize_background_state(background)?;
    *state
        .background_draft
        .lock()
        .map_err(|_| "background lock poisoned")? = Some(normalized);
    let _ = app.emit("background-preview", ());
    Ok(())
}

/// 保存：写 `background.dat` + 背景参数，清掉草稿。
#[tauri::command]
pub fn commit_background(
    app: AppHandle,
    state: State<'_, AppState>,
    background: BackgroundState,
) -> Result<(), String> {
    let normalized = normalize_background_state(background)?;
    match &normalized.image {
        Some(data) => std::fs::write(background_path(), data).map_err(|err| err.to_string())?,
        None => match std::fs::remove_file(background_path()) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.to_string()),
        },
    }
    {
        let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
        for (key, value) in [
            ("bg_ratio", normalized.ratio),
            ("bg_dx", normalized.dx),
            ("bg_dy", normalized.dy),
            ("bg_blur", normalized.blur),
            ("bg_dim", normalized.dim),
        ] {
            store
                .set(key, Value::from(value))
                .map_err(|err| err.to_string())?;
        }
    }
    *state
        .background_draft
        .lock()
        .map_err(|_| "background lock poisoned")? = None;
    let _ = app.emit("background-changed", ());
    Ok(())
}

/// 取消预览：丢弃草稿，所有窗口回到已保存效果。
#[tauri::command]
pub fn cancel_background_preview(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    *state
        .background_draft
        .lock()
        .map_err(|_| "background lock poisoned")? = None;
    let _ = app.emit("background-preview", ());
    Ok(())
}

#[tauri::command]
pub fn show_background_editor(app: AppHandle) {
    if let Err(err) = window_mgr::show_bg_editor(&app) {
        eprintln!("[show_background_editor] {err}");
    }
}

#[tauri::command]
pub fn hide_background_editor(app: AppHandle) {
    window_mgr::hide_bg_editor(&app);
}

// ---------- 实物卡片图片（`card-images/{id}.dat`） ----------

/// 设备 id 已是 slug 形式，这里再过滤一次，禁止路径分隔符进文件名。
fn card_image_name(id: &str) -> String {
    let clean: String = id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .collect();
    if clean.is_empty() {
        "device".to_string()
    } else {
        clean
    }
}

fn card_image_path(id: &str) -> std::path::PathBuf {
    config::app_dir()
        .join("card-images")
        .join(format!("{}.dat", card_image_name(id)))
}

/// 保存某台设备的卡片图片：入参为整段 `data:image/...;base64,...`。
#[tauri::command]
pub fn set_device_card_image(app: AppHandle, id: String, data: String) -> Result<(), String> {
    let trimmed = data.trim();
    if !trimmed.starts_with("data:image/") {
        return Err("卡片图片必须是 data URL 图片".into());
    }
    if trimmed.len() > 8 * 1024 * 1024 {
        return Err("卡片图片过大（原图请不超过 6 MB）".into());
    }
    let path = card_image_path(&id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(path, trimmed).map_err(|err| err.to_string())?;
    let _ = app.emit("card-image-changed", id);
    Ok(())
}

/// 读取某台设备的卡片图片；没配过返回 `None`。
#[tauri::command]
pub fn get_device_card_image(id: String) -> Result<Option<String>, String> {
    match std::fs::read_to_string(card_image_path(&id)) {
        Ok(text) => {
            let trimmed = text.trim();
            Ok(trimmed
                .starts_with("data:image/")
                .then(|| trimmed.to_string()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

/// 清除某台设备的卡片图片（恢复内置图片 / 矢量剪影）。
#[tauri::command]
pub fn clear_device_card_image(app: AppHandle, id: String) -> Result<(), String> {
    match std::fs::remove_file(card_image_path(&id)) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.to_string()),
    }
    let _ = app.emit("card-image-changed", id);
    Ok(())
}

/// 按魔数识别图片格式（顺带挡掉把非图文件当图存进来）。
fn image_mime(bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Ok("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok("image/jpeg");
    }
    if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok("image/webp");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("image/gif");
    }
    if bytes.starts_with(b"BM") {
        return Ok("image/bmp");
    }
    Err("不是支持的图片格式（png / jpg / webp / gif / bmp）".to_string())
}

/// 自带 base64：只为把文件转成 data URL，不值得为此引一个依赖。
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: [u8; 64] = *b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(TABLE[b0 >> 2] as char);
        out.push(TABLE[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((b1 & 0x0F) << 2) | (b2 >> 6)] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[b2 & 0x3F] as char
        } else {
            '='
        });
    }
    out
}

/// 从路径导入时把超大图缩到最长边 1024，避免 WebView 对几千万像素的图实时缩放发糊。
/// 解码失败就原样返回（仍受 8 MB 上限约束）。
fn shrink_card_image(bytes: &[u8], mime: &'static str) -> Result<(Vec<u8>, &'static str), String> {
    let Ok(img) = image::load_from_memory(bytes) else {
        return Ok((bytes.to_vec(), mime));
    };
    if img.width() <= 1024 && img.height() <= 1024 {
        return Ok((bytes.to_vec(), mime));
    }
    let resized = img.resize(1024, 1024, image::imageops::FilterType::Lanczos3);
    let mut out = Vec::new();
    let keep_alpha = mime != "image/jpeg" && mime != "image/bmp";
    if keep_alpha {
        use std::io::Cursor;
        let mut cursor = Cursor::new(out);
        resized
            .write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|err| format!("图片缩放失败：{err}"))?;
        out = cursor.into_inner();
        if out.len() <= 5 * 1024 * 1024 {
            return Ok((out, "image/png"));
        }
    }
    let rgb = resized.to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88)
        .encode_image(&rgb)
        .map_err(|err| format!("图片编码失败：{err}"))?;
    Ok((out, "image/jpeg"))
}

/// 从磁盘路径装入卡片图片：绕开 WebView 的 File API / canvas，作为取图兜底。
///
/// 超过 1024 的长边在 Rust 侧先缩小再落盘；原图上限 5 MB。
#[tauri::command]
pub fn set_device_card_image_from_path(
    app: AppHandle,
    id: String,
    path: String,
) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("请填写图片文件的完整路径".into());
    }
    let bytes = std::fs::read(std::path::PathBuf::from(trimmed))
        .map_err(|err| format!("读取图片失败：{err}"))?;
    if bytes.is_empty() {
        return Err("文件是空的".into());
    }
    if bytes.len() > 5 * 1024 * 1024 {
        return Err("原图超过 5 MB：请先压缩，或点「选择图片」（会自动缩图）".into());
    }
    let mime = image_mime(&bytes)?;
    let (shrunk, mime) = shrink_card_image(&bytes, mime)?;
    let data = format!("data:{mime};base64,{}", base64_encode(&shrunk));
    set_device_card_image(app, id, data)
}

/// 弹系统「打开文件」对话框选一张图，返回完整路径（取消返回 `None`）。
///
/// WebView 的 `<input type="file">` 在无边框透明窗口里有时会把对话框开到主窗后面，
/// 表现为「点了没反应」；这里用 Win32 `GetOpenFileNameW`，owner 挂在主窗句柄上，
/// 一定出现在最前。选完由前端走 `set_device_card_image_from_path` 落盘。
#[tauri::command]
pub fn pick_card_image_file(window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    use windows::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_PATHMUSTEXIST,
        OPENFILENAMEW,
    };
    use windows::core::PCWSTR;

    let owner = window.hwnd().map_err(|err| err.to_string())?;
    let mut buffer = [0u16; 4096];
    // 过滤器里的项用 \0 分隔、整串用 \0\0 结尾
    let filter: Vec<u16> = "图片文件\0*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp\0所有文件\0*.*\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "选择卡片图片\0".encode_utf16().collect();

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: windows::Win32::Foundation::HWND(owner.0),
        lpstrFilter: PCWSTR(filter.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: windows::core::PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,
        ..Default::default()
    };

    unsafe {
        if !GetOpenFileNameW(&mut ofn).as_bool() {
            return Ok(None); // 用户取消，不是错误
        }
    }
    let len = buffer
        .iter()
        .position(|&ch| ch == 0)
        .unwrap_or(buffer.len());
    let path = String::from_utf16_lossy(&buffer[..len]);
    if path.is_empty() {
        return Ok(None);
    }
    Ok(Some(path))
}

/// 设置页「测试」：立即闪一次红色闪电，时长取托盘闪烁设置。
#[tauri::command]
pub fn test_tray_blink(app: AppHandle) {
    let seconds = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .store
                .lock()
                .ok()
                .map(|store| store.tray_blink_seconds())
        })
        .unwrap_or(10);
    crate::tray::start_blink(&app, seconds);
}

#[derive(Debug, PartialEq, Eq)]
enum ResetOverlayLayout {
    Popup,
    Tooltip,
    AlertPreview,
}

fn reset_overlay_layouts() -> [ResetOverlayLayout; 3] {
    [
        ResetOverlayLayout::Popup,
        ResetOverlayLayout::Tooltip,
        ResetOverlayLayout::AlertPreview,
    ]
}

#[tauri::command]
pub fn reset_settings(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|_| "config lock poisoned")?;
    for (key, value) in config::DEFAULTS.iter() {
        if key != "devices" {
            store
                .set(key, value.clone())
                .map_err(|err| err.to_string())?;
        }
    }
    drop(store);
    refresh::reschedule(&app);
    // 页面背景一并恢复默认：删掉自定义壁纸，回到内置壁纸
    let _ = std::fs::remove_file(background_path());
    if let Ok(mut draft) = state.background_draft.lock() {
        *draft = None;
    }
    sync_autostart(false);
    window_mgr::apply_main_geometry(&app);
    for layout in reset_overlay_layouts() {
        match layout {
            ResetOverlayLayout::Popup => window_mgr::relayout_popup(&app),
            ResetOverlayLayout::Tooltip => window_mgr::relayout_tooltip(&app),
            ResetOverlayLayout::AlertPreview => relayout_alert_preview(&app),
        }
    }
    reload_hotkey(&app);
    crate::theme::apply(&app);
    crate::theme::apply_icon_for_current(&app);
    crate::tray::refresh_battery_icon(&app);
    let _ = app.emit("config-changed", ());
    let _ = app.emit("background-changed", ());
    Ok(())
}

#[tauri::command]
pub fn show_settings(app: AppHandle) {
    window_mgr::show_settings(&app);
}

fn reload_hotkey(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let (enabled, accelerator) = match state.store.lock() {
        Ok(store) => (
            store
                .get("hotkey_enabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            store
                .get("hotkey")
                .and_then(|value| value.as_str())
                .unwrap_or("ctrl+alt+d")
                .to_string(),
        ),
        Err(_) => return,
    };
    if enabled {
        let _ = register_hotkey(app.clone(), accelerator);
    } else {
        unregister_hotkey(app.clone());
    }
}

#[cfg(not(debug_assertions))]
fn sync_autostart(enabled: bool) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = crate::autostart::sync(enabled, &exe);
    }
}

#[cfg(debug_assertions)]
fn sync_autostart(_enabled: bool) {}
