use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::AppState;
use crate::config;

pub fn start(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("vxe-theme".to_string())
        .spawn(move || {
            loop {
                apply(&app);
                std::thread::sleep(Duration::from_secs(5));
            }
        });
}

pub fn apply(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let want = {
        let Ok(store) = state.store.lock() else {
            return;
        };
        match store.theme_mode() {
            config::ThemeMode::Manual => None,
            config::ThemeMode::System => Some(if system_is_light() { "light" } else { "dark" }),
            config::ThemeMode::Schedule => Some(schedule_theme(&store)),
        }
    };
    let Some(want) = want else {
        return;
    };
    {
        if let Ok(mut store) = state.store.lock() {
            if store.ui_theme().as_str() == want {
                return;
            }
            let _ = store.set("ui_theme", Value::from(want));
        }
    }
    apply_icon_for_current(app);
    let _ = app.emit("theme-changed", want);
    let _ = app.emit("config-changed", ());
}

pub fn apply_icon_for_current(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let light = state
        .store
        .lock()
        .map(|store| store.ui_theme().as_str() == "light")
        .unwrap_or(false);
    let icon = if light {
        tauri::image::Image::from_bytes(include_bytes!("../icons/app_light.ico"))
    } else {
        tauri::image::Image::from_bytes(include_bytes!("../icons/app_dark.ico"))
    };
    if let Ok(icon) = icon
        && let Some(window) = app.get_webview_window("main")
    {
        let _ = window.set_icon(icon);
    }
}

fn schedule_theme(store: &config::Store) -> &'static str {
    let now = local_minutes();
    let light = store
        .get("theme_light_at")
        .and_then(Value::as_str)
        .and_then(parse_minutes)
        .unwrap_or(7 * 60);
    let dark = store
        .get("theme_dark_at")
        .and_then(Value::as_str)
        .and_then(parse_minutes)
        .unwrap_or(19 * 60);
    if light == dark {
        return if now >= light { "light" } else { "dark" };
    }
    if light < dark {
        if light <= now && now < dark {
            "light"
        } else {
            "dark"
        }
    } else if now >= light || now < dark {
        "light"
    } else {
        "dark"
    }
}

fn parse_minutes(raw: &str) -> Option<i32> {
    let text = raw.trim().replace('：', ":");
    let (hour, minute) = text.split_once(':')?;
    let hour = hour.parse::<i32>().ok()?;
    let minute = minute.parse::<i32>().ok()?;
    ((0..24).contains(&hour) && (0..60).contains(&minute)).then_some(hour * 60 + minute)
}

fn local_minutes() -> i32 {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::GetLocalTime;
        // SAFETY: GetLocalTime has no pointer arguments and returns a plain value.
        let time = unsafe { GetLocalTime() };
        i32::from(time.wHour) * 60 + i32::from(time.wMinute)
    }
    #[cfg(not(windows))]
    {
        12 * 60
    }
}

fn system_is_light() -> bool {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE};

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            KEY_QUERY_VALUE,
        )
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|value| value != 0)
        .unwrap_or(false)
}
