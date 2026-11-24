use std::sync::Mutex;
use std::time::Duration;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::battery::{BatteryState, DeviceStatus};
use crate::commands::AppState;
use crate::tray_icon;

const ICON_SIZE: u32 = tray_icon::ICON_SIZE;
const BLINK_INTERVAL_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IconKey {
    style: tray_icon::IconStyle,
    level: u8,
    tier: tray_icon::Tier,
    neutral: bool,
}

#[derive(Debug)]
struct BlinkState {
    epoch: u64,
    active: bool,
    icon_key: Option<IconKey>,
}

static BLINK: Mutex<BlinkState> = Mutex::new(BlinkState {
    epoch: 0,
    active: false,
    icon_key: None,
});

fn current_style(app: &AppHandle) -> tray_icon::IconStyle {
    app.try_state::<AppState>()
        .and_then(|state| {
            state
                .store
                .lock()
                .ok()
                .map(|store| tray_icon::IconStyle::parse(store.tray_icon_style()))
        })
        .unwrap_or(tray_icon::IconStyle::Bolt)
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("VXE")
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                let app = tray.app_handle();
                // E5：Windows 双击会送两次左键 up，200 ms 内只处理第一次。
                let should_show = if let Some(state) = app.try_state::<crate::commands::AppState>()
                {
                    let Ok(mut last) = state.last_tray_click.lock() else {
                        return;
                    };
                    let now = std::time::Instant::now();
                    let debounce = last.is_some_and(|previous| {
                        now.duration_since(previous) < Duration::from_millis(200)
                    });
                    if !debounce {
                        *last = Some(now);
                    }
                    !debounce
                } else {
                    true
                };
                if should_show {
                    let shown = crate::window_mgr::show_popup(app, false);
                    tray_log(&format!("left-click shown={shown}"));
                    if !shown {
                        // 预览窗异常时退化为直接打开面板，避免“点了没反应”
                        crate::window_mgr::show_main(app, "summary");
                    }
                }
            }
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Up,
                ..
            } => {
                tray_log("right-click");
                crate::window_mgr::show_tray_menu(tray.app_handle())
            }
            _ => {}
        });

    let style = current_style(app);
    tray_log(&format!("create style={}", style.as_str()));
    builder = builder.icon(image(tray_icon::render_neutral(style)));
    let tray = builder.build(app)?;
    app.manage(tray);
    Ok(())
}

fn image(rgba: Vec<u8>) -> tauri::image::Image<'static> {
    tauri::image::Image::new_owned(rgba, ICON_SIZE, ICON_SIZE)
}

/// 托盘事件落盘到 exe 同级 `tray.log`，用于诊断“偶发点击无反应”。
pub(crate) fn tray_log(line: &str) {
    use std::io::Write;
    let path = crate::config::app_dir().join("tray.log");
    if let Ok(meta) = std::fs::metadata(&path)
        && meta.len() > 64 * 1024
    {
        let _ = std::fs::write(&path, "");
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "{stamp} {line}");
    }
}

fn set_icon(app: &AppHandle, rgba: Vec<u8>) {
    if let Some(tray) = app.try_state::<tauri::tray::TrayIcon<tauri::Wry>>() {
        let _ = tray.set_icon(Some(image(rgba)));
    }
}

fn set_icon_if_idle(app: &AppHandle, rgba: Vec<u8>, key: IconKey) -> bool {
    let Ok(mut state) = BLINK.lock() else {
        return false;
    };
    if state.active {
        return false;
    }
    set_icon(app, rgba);
    state.icon_key = Some(key);
    true
}

fn icon_key_matches(key: IconKey) -> bool {
    BLINK
        .lock()
        .map(|state| state.icon_key == Some(key))
        .unwrap_or(false)
}

/// 托盘图标的电量：实时读数优先，休眠/掉线退回上次读数（和总览 Hero 一致）。
fn display_level(status: &DeviceStatus) -> Option<u8> {
    if status.state == BatteryState::Ok && status.level.is_some() {
        status.level
    } else {
        status.last
    }
}

/// 所有设备展示电量的平均值；没有读数时返回 None。半数舍入取偶，与前端 `stats` 一致。
#[must_use]
pub fn average_level(statuses: &[DeviceStatus]) -> Option<u8> {
    let levels = statuses
        .iter()
        .filter_map(display_level)
        .map(i32::from)
        .collect::<Vec<_>>();
    if levels.is_empty() {
        return None;
    }
    let count = levels.len() as i32;
    let sum: i32 = levels.iter().sum();
    let quotient = sum / count;
    let remainder = sum % count;
    let rounded = if remainder * 2 > count || (remainder * 2 == count && quotient % 2 != 0) {
        quotient + 1
    } else {
        quotient
    };
    Some(rounded.clamp(0, 100) as u8)
}

/// 所有设备展示电量里的最低值；没有读数时返回 None。
#[must_use]
pub fn min_level(statuses: &[DeviceStatus]) -> Option<u8> {
    statuses.iter().filter_map(display_level).min()
}

/// 按设置页选择取托盘电量：平均值 / 指定设备 / 最低电量。
#[must_use]
pub fn level_for_source(statuses: &[DeviceStatus], mode: &str, device: &str) -> Option<u8> {
    match mode {
        "device" => match statuses.iter().find(|status| status.id == device) {
            Some(status) => display_level(status),
            None => average_level(statuses),
        },
        "min" => min_level(statuses),
        _ => average_level(statuses),
    }
}

/// 刷新后按当前电量更新托盘图标。
pub fn update_for_statuses(
    app: &AppHandle,
    statuses: &[DeviceStatus],
    good: i32,
    warn: i32,
    mode: &str,
    device: &str,
    style: &str,
) {
    let style = tray_icon::IconStyle::parse(style);
    let key = match level_for_source(statuses, mode, device) {
        Some(level) => {
            let tier =
                tray_icon::Tier::of(level, good.clamp(0, 100) as u8, warn.clamp(0, 100) as u8);
            IconKey {
                style,
                level: tray_icon::quantize(level),
                tier,
                neutral: false,
            }
        }
        None => IconKey {
            style,
            level: 0,
            tier: tray_icon::Tier::Low,
            neutral: true,
        },
    };
    if icon_key_matches(key) {
        return;
    }
    let icon = if key.neutral {
        tray_icon::render_neutral(style)
    } else {
        tray_icon::render_style(style, key.level, key.tier.color())
    };
    set_icon_if_idle(app, icon, key);
}

/// 设置页改电量区间后立即重算图标，不必等下一次采集。
pub fn refresh_battery_icon(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let statuses = state
        .statuses
        .lock()
        .map(|statuses| statuses.clone())
        .unwrap_or_default();
    let (good, warn, mode, device, style) = state
        .store
        .lock()
        .map(|store| {
            (
                store.good_level(),
                store.warn_level(),
                store.tray_level_mode(),
                store.tray_level_device(),
                store.tray_icon_style(),
            )
        })
        .unwrap_or((80, 20, "average", String::new(), "bolt"));
    update_for_statuses(app, &statuses, good, warn, mode, &device, style);
}

/// 低电量提醒：按当前样式闪全红，返回设置里的时长后恢复当前电量图标。
pub fn start_blink(app: &AppHandle, seconds: u64) {
    let style = current_style(app);
    let epoch = {
        let Ok(mut state) = BLINK.lock() else {
            return;
        };
        state.epoch = state.epoch.wrapping_add(1);
        state.active = true;
        state.icon_key = None;
        state.epoch
    };
    tray_log(&format!("blink style={}", style.as_str()));

    let on = tray_icon::render_alert_on(style);
    let off = tray_icon::render_alert_off(style);
    set_icon(app, on.clone());

    let app = app.clone();
    let steps = seconds.clamp(1, 600) * 2;
    let _ = std::thread::Builder::new()
        .name("vxe-tray-blink".to_string())
        .spawn(move || {
            for step in 0..steps {
                std::thread::sleep(Duration::from_millis(BLINK_INTERVAL_MS));
                {
                    let Ok(state) = BLINK.lock() else {
                        return;
                    };
                    if state.epoch != epoch {
                        return;
                    }
                    set_icon(
                        &app,
                        if step % 2 == 0 {
                            off.clone()
                        } else {
                            on.clone()
                        },
                    );
                }
            }
            let finished = {
                let Ok(mut state) = BLINK.lock() else {
                    return;
                };
                if state.epoch == epoch {
                    state.active = false;
                    true
                } else {
                    false
                }
            };
            if finished {
                refresh_battery_icon(&app);
            }
        });
}

/// Win11: `IsPromoted=0` means the icon lives in the "^" overflow panel; the
/// shell still reports a phantom rect, so callers must not treat it as hoverable.
pub fn is_promoted() -> Option<bool> {
    // hover 每 80ms 查询；注册表（含未找到）最多每两秒刷新一次。
    static CACHE: Mutex<Option<(std::time::Instant, Option<bool>)>> = Mutex::new(None);
    let mut cache = CACHE.lock().ok()?;
    if let Some((at, value)) = *cache
        && at.elapsed() < Duration::from_secs(2)
    {
        return value;
    }
    let value = query_promoted();
    *cache = Some((std::time::Instant::now(), value));
    value
}

fn normalize_executable_path(path: &str) -> String {
    let path = path
        .strip_prefix(r"\\?\UNC\")
        .map(|path| format!(r"\\{path}"))
        .or_else(|| path.strip_prefix(r"\\?\").map(str::to_owned))
        .unwrap_or_else(|| path.to_owned());
    path.to_ascii_lowercase()
}

fn query_promoted() -> Option<bool> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_ENUMERATE_SUB_KEYS, KEY_QUERY_VALUE};
    static EXE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let exe_text = EXE
        .get_or_init(|| {
            let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
            Some(normalize_executable_path(&exe.to_string_lossy()))
        })
        .as_ref()?;
    let root = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(
            r"Control Panel\NotifyIconSettings",
            KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS,
        )
        .ok()?;
    for name in root.enum_keys() {
        let name = match name {
            Ok(name) => name,
            Err(error) => {
                eprintln!("[tray] enumerate NotifyIconSettings failed: {error}");
                return None;
            }
        };
        let Ok(sub) = root.open_subkey_with_flags(&name, KEY_QUERY_VALUE) else {
            continue;
        };
        let Ok(path) = sub.get_value::<String, _>("ExecutablePath") else {
            continue;
        };
        if normalize_executable_path(&path) == *exe_text {
            return sub.get_value::<u32, _>("IsPromoted").ok().map(|v| v != 0);
        }
    }
    None
}

pub fn set_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.try_state::<tauri::tray::TrayIcon<tauri::Wry>>() {
        let _ = tray.set_tooltip(Some(text));
    }
}
