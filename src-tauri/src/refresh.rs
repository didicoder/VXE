use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::battery::{self, BatteryState, DeviceSpec, DeviceStatus};
use crate::commands::AppState;
use crate::config;
use crate::sound;
use crate::window_mgr;

#[derive(Debug, Clone, Serialize)]
pub struct AlertPayload {
    pub device: String,
    pub battery: u8,
    pub color: Option<String>,
}

/// 低电量提醒的每设备状态：是否已武装、上次提醒时间（仅内存）。
#[derive(Debug, Clone, Copy)]
pub struct LowNotifyState {
    pub armed: bool,
    pub last_alert: Option<Instant>,
}

impl LowNotifyState {
    pub fn fresh() -> Self {
        Self {
            armed: true,
            last_alert: None,
        }
    }
}

/// 单轮低电量状态机结果；`Rearm` 同时清空重复提醒计时。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LowNotifyAction {
    Quiet,
    Notify,
    Rearm,
}

fn low_notify_action(
    level: i32,
    threshold: i32,
    charging: bool,
    armed: bool,
    since_last_alert: Option<Duration>,
    interval: Duration,
) -> LowNotifyAction {
    if level >= threshold + 3 {
        return LowNotifyAction::Rearm;
    }
    if charging || level > threshold {
        return LowNotifyAction::Quiet;
    }
    if armed {
        return LowNotifyAction::Notify;
    }
    match since_last_alert {
        Some(elapsed) if !interval.is_zero() && elapsed >= interval => LowNotifyAction::Notify,
        _ => LowNotifyAction::Quiet,
    }
}

/// 原生后台是唯一自动采集调度器，界面不存在时也继续工作。
#[derive(Default)]
pub struct Scheduler {
    revision: Mutex<u64>,
    changed: Condvar,
    last_result: Mutex<Option<Result<Vec<DeviceStatus>, String>>>,
}

pub fn reschedule(app: &AppHandle) {
    if let Some(scheduler) = app.try_state::<Scheduler>()
        && let Ok(mut revision) = scheduler.revision.lock()
    {
        *revision = revision.wrapping_add(1);
        scheduler.changed.notify_one();
    }
}

pub fn start(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("vxe-refresh".to_string())
        .spawn(move || {
            loop {
                let _ = read_and_store(&app);
                let scheduler = app.state::<Scheduler>();
                let Ok(mut revision) = scheduler.revision.lock() else {
                    return;
                };
                loop {
                    let seconds = app
                        .try_state::<AppState>()
                        .and_then(|state| {
                            state.store.lock().ok().map(|store| store.refresh_seconds())
                        })
                        .unwrap_or(60)
                        .max(1);
                    let observed = *revision;
                    let Ok((next, timeout)) = scheduler.changed.wait_timeout_while(
                        revision,
                        Duration::from_secs(seconds),
                        |current| *current == observed,
                    ) else {
                        return;
                    };
                    revision = next;
                    if timeout.timed_out() {
                        break;
                    }
                    // 手动刷新或间隔设置变更后，按最新配置重新计时。
                }
            }
        });
}

pub fn request_manual(app: AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = manual_refresh(&app);
    });
}

pub fn manual_refresh(app: &AppHandle) -> Result<Vec<DeviceStatus>, String> {
    if let Some(main) = app.get_webview_window(window_mgr::MAIN)
        && main.is_visible().unwrap_or(false)
    {
        let _ = main.emit("refresh-started", ());
    }
    let result = read_and_store(app);
    let _ = app.emit(
        "refresh-done",
        serde_json::json!({"error": result.as_ref().err()}),
    );
    reschedule(app);
    result
}

pub fn read_and_store(app: &AppHandle) -> Result<Vec<DeviceStatus>, String> {
    let state = app.try_state::<AppState>().ok_or("state missing")?;
    let scheduler = app.state::<Scheduler>();
    let _refresh_guard = match state.refresh_lock.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            // 与在途采集合并：等待真实结果，不能立即返回上一轮旧缓存。
            let _guard = state
                .refresh_lock
                .lock()
                .map_err(|_| "refresh lock poisoned")?;
            return scheduler
                .last_result
                .lock()
                .map_err(|_| "refresh result lock poisoned")?
                .clone()
                .unwrap_or_else(|| current_statuses(&state));
        }
        Err(_) => return Err("refresh lock poisoned".to_string()),
    };
    let result = collect_and_store(app, &state);
    if let Ok(mut last) = scheduler.last_result.lock() {
        *last = Some(result.clone());
    }
    result
}

fn collect_and_store(app: &AppHandle, state: &AppState) -> Result<Vec<DeviceStatus>, String> {
    let (
        specs,
        profile_dirs,
        log_dir,
        mut last_levels,
        notify_low,
        notify_level,
        notify_interval_minutes,
        good,
        warn,
        band_colors,
        alert_sound,
        sound_file,
        alert_mode,
        tray_blink_seconds,
        tray_level_mode,
        tray_level_device,
    ) = {
        let store = state.store.lock().map_err(|_| "config lock poisoned")?;
        let last_levels = store
            .get("last_levels")
            .and_then(Value::as_object)
            .map(|levels| {
                levels
                    .iter()
                    .filter_map(|(id, value)| {
                        let level = value.as_u64()?;
                        (level <= 100).then_some((id.clone(), level as u8))
                    })
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let band_colors = store
            .get("band_colors")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        (
            store
                .devices()
                .iter()
                .map(DeviceSpec::from_json)
                .collect::<Vec<_>>(),
            profile_dirs(),
            config::app_dir().to_path_buf(),
            last_levels,
            value_truthy(store.get("notify_low")),
            store.notify_level(),
            store.notify_interval_minutes(),
            store.good_level(),
            store.warn_level(),
            band_colors,
            store.alert_sound(),
            store.alert_sound_file(),
            store.alert_mode(),
            store.tray_blink_seconds(),
            store.tray_level_mode(),
            store.tray_level_device(),
        )
    };

    let mut statuses = battery::read_all(&specs, &profile_dirs, Some(&log_dir));
    let mut levels_changed = false;
    for status in &mut statuses {
        let spec = specs.iter().find(|spec| spec.id == status.id);
        if status.state == BatteryState::Ok
            && let Some(level) = status.level
        {
            if last_levels.get(&status.id).copied() != Some(level) {
                last_levels.insert(status.id.clone(), level);
                levels_changed = true;
            }
            status.last = Some(level);
            continue;
        }
        if spec.is_some_and(battery::is_audio) {
            status.last = None;
        } else {
            status.last = last_levels.get(&status.id).copied();
        }
    }

    if levels_changed && let Ok(mut store) = state.store.lock() {
        let levels = last_levels
            .iter()
            .map(|(id, level)| (id.clone(), Value::from(*level)))
            .collect::<Map<String, Value>>();
        let _ = store.set("last_levels", Value::Object(levels));
    }

    {
        let mut guard = state.statuses.lock().map_err(|_| "status lock poisoned")?;
        *guard = statuses.clone();
    }
    crate::tray::set_tooltip(app, &tip_text(&statuses));
    crate::tray::update_for_statuses(
        app,
        &statuses,
        good,
        warn,
        tray_level_mode,
        &tray_level_device,
    );
    let _ = app.emit("devices-updated", &statuses);
    crate::window_mgr::relayout_popup(app);
    crate::window_mgr::relayout_tooltip(app);

    if notify_low {
        let now = Instant::now();
        let interval = Duration::from_secs((notify_interval_minutes as u64) * 60);
        let pending = {
            let mut states = state
                .low_notify
                .lock()
                .map_err(|_| "low-battery lock poisoned")?;
            let mut pending = Vec::new();
            for status in &statuses {
                if status.state != BatteryState::Ok {
                    continue;
                }
                let Some(level) = status.level else {
                    continue;
                };
                let entry = states
                    .entry(status.id.clone())
                    .or_insert_with(LowNotifyState::fresh);
                let since_last = entry.last_alert.map(|at| now.saturating_duration_since(at));
                match low_notify_action(
                    i32::from(level),
                    notify_level,
                    status.charging,
                    entry.armed,
                    since_last,
                    interval,
                ) {
                    LowNotifyAction::Rearm => {
                        entry.armed = true;
                        entry.last_alert = None;
                    }
                    LowNotifyAction::Notify => {
                        entry.armed = false;
                        entry.last_alert = Some(now);
                        let spec = specs.iter().find(|spec| spec.id == status.id);
                        let color = spec
                            .and_then(|spec| alert_color(spec, level, good, warn, &band_colors));
                        pending.push(AlertPayload {
                            device: status.label.clone(),
                            battery: level,
                            color,
                        });
                    }
                    LowNotifyAction::Quiet => {}
                }
            }
            pending
        };
        for payload in pending {
            // 提醒方式：popup = 弹窗（含提示音），tray = 托盘图标闪烁，both = 两者
            if alert_mode != "tray" {
                enqueue_alert(app, payload.clone(), alert_sound, &sound_file);
            }
            if alert_mode != "popup" {
                crate::tray::start_blink(app, tray_blink_seconds);
            }
        }
    }

    Ok(statuses)
}

pub fn enqueue_alert(app: &AppHandle, payload: AlertPayload, play_sound: bool, sound_file: &str) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let mut next = None;
    {
        let Ok(mut queue) = state.alerts.lock() else {
            return;
        };
        queue.retain(|item| item.device != payload.device);
        queue.push_back(payload.clone());
        while queue.len() > 3 {
            queue.pop_front();
        }
        if !state.alert_showing.swap(true, Ordering::SeqCst) {
            next = queue.pop_front();
            if next.is_none() {
                state.alert_showing.store(false, Ordering::SeqCst);
            }
        }
    }
    if let Some(next) = next {
        show_alert(app, next, play_sound, sound_file);
    }
}

fn show_alert(app: &AppHandle, payload: AlertPayload, play_sound: bool, sound_file: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.alert_preview.store(false, Ordering::SeqCst);
        if let Ok(mut current) = state.current_alert.lock() {
            *current = Some(payload.clone());
        }
    }
    window_mgr::show_alert_event(app, "alert-show", serde_json::json!(payload));
    let app = app.clone();
    let sound_file = sound_file.to_string();
    let _ = std::thread::Builder::new()
        .name("vxe-alert-queue".to_string())
        .spawn(move || {
            // 冷创建完成、事件订阅就绪后才开始完整提醒时间线。
            if window_mgr::wait_alert_visible(&app, Duration::from_secs(15)) {
                if play_sound {
                    play_alert_sound(&app, &sound_file);
                }
                std::thread::sleep(Duration::from_millis(5850));
            } else {
                window_mgr::hide(&app, window_mgr::ALERT);
            }
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            let next = {
                let Ok(mut queue) = state.alerts.lock() else {
                    return;
                };
                let next = queue.pop_front();
                if next.is_none() {
                    state.alert_showing.store(false, Ordering::SeqCst);
                    if let Ok(mut current) = state.current_alert.lock() {
                        *current = None;
                    }
                    window_mgr::hide(&app, window_mgr::ALERT);
                }
                next
            };
            if let Some(next) = next {
                show_alert(&app, next, play_sound, &sound_file);
            }
        });
}

fn play_alert_sound(app: &AppHandle, requested: &str) {
    play_sound_file(app, requested);
}

/// 试听/告警共用：按 exe → 资源 → 开发目录顺序找 wav，缺失时系统提示音。
pub fn play_sound_file(app: &AppHandle, requested: &str) {
    let resource_dir = app.path().resource_dir().ok();
    #[cfg(debug_assertions)]
    let dev_dir = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources"));
    #[cfg(not(debug_assertions))]
    let dev_dir: Option<PathBuf> = None;
    let dirs = sound::search_dirs(
        Some(&config::program_dir()),
        resource_dir.as_deref(),
        dev_dir.as_deref(),
    );
    // 外部 sound/ 同名文件优先，没有就播内置（随二进制打包）
    sound::play_requested(&dirs, requested);
}

/// 设置页预览/定位用的示例：第一台设备的名称 + 通知阈值下方 5%。
pub fn sample_alert(app: &AppHandle) -> AlertPayload {
    let Some(state) = app.try_state::<AppState>() else {
        return AlertPayload {
            device: "示例设备".to_string(),
            battery: 15,
            color: None,
        };
    };
    let device = state
        .statuses
        .lock()
        .ok()
        .and_then(|statuses| statuses.first().map(|status| status.label.clone()))
        .unwrap_or_else(|| "示例设备".to_string());
    let level = state
        .store
        .lock()
        .ok()
        .map(|store| store.notify_level())
        .unwrap_or(20);
    AlertPayload {
        device,
        battery: (level - 5).clamp(5, 99) as u8,
        color: None,
    }
}

fn current_statuses(state: &AppState) -> Result<Vec<DeviceStatus>, String> {
    state
        .statuses
        .lock()
        .map(|statuses| statuses.clone())
        .map_err(|_| "status lock poisoned".to_string())
}

fn tip_text(statuses: &[DeviceStatus]) -> String {
    if statuses.is_empty() {
        return "VXE".to_string();
    }
    statuses
        .iter()
        .map(|status| {
            let short = status.label.split(' ').next().unwrap_or(&status.id);
            match (status.state, status.level) {
                (BatteryState::Ok, Some(level)) => {
                    let tail = if status.charging { " 充电中" } else { "" };
                    format!("{short} {level}%{tail}")
                }
                (_, _)
                    if status.charging
                        && matches!(status.state, BatteryState::Ok | BatteryState::Sleep) =>
                {
                    format!("{short} 充电中")
                }
                (BatteryState::Sleep, _) => format!("{short} 休眠"),
                (BatteryState::Absent, _) => format!("{short} 未连接"),
                _ => format!("{short} --"),
            }
        })
        .collect::<Vec<_>>()
        .join(" ｜ ")
}

fn alert_color(
    spec: &DeviceSpec,
    level: u8,
    good: i32,
    warn: i32,
    band_colors: &Map<String, Value>,
) -> Option<String> {
    let band = if i32::from(level) >= good {
        "good"
    } else if i32::from(level) >= warn {
        "mid"
    } else {
        "low"
    };
    spec.colors
        .get(band)
        .and_then(Value::as_str)
        .or_else(|| band_colors.get(band).and_then(Value::as_str))
        .map(str::to_owned)
        .or_else(|| {
            Some(
                match band {
                    "good" => "green",
                    "mid" => "blue",
                    _ => "red",
                }
                .to_string(),
            )
        })
}

fn value_truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::Null) | None => false,
        Some(Value::Number(value)) => value.as_f64().is_some_and(|number| number != 0.0),
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::Object(value)) => !value.is_empty(),
    }
}

pub fn profile_dirs() -> Vec<PathBuf> {
    #[cfg(debug_assertions)]
    {
        vec![
            config::program_dir().join("device_profiles"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/device_profiles"),
        ]
    }
    #[cfg(not(debug_assertions))]
    {
        vec![config::program_dir().join("device_profiles")]
    }
}
