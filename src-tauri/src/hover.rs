use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::commands::AppState;
use crate::window_mgr;

pub fn start(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("vxe-hover".to_string())
        .spawn(move || {
            let mut inside_since: Option<Instant> = None;
            loop {
                std::thread::sleep(Duration::from_millis(80));
                let Some(state) = app.try_state::<AppState>() else {
                    continue;
                };
                if state.tooltip_hold.load(Ordering::SeqCst) {
                    inside_since = None;
                    continue;
                }
                // 预览/托盘都可能改过可见性，不能依赖线程内的旧标记。
                let tooltip_visible = window_visible(&app, window_mgr::TOOLTIP);
                let tooltip_closing = state.tooltip_closing.load(Ordering::SeqCst);
                let scale = app
                    .get_webview_window(window_mgr::MAIN)
                    .and_then(|window| window.scale_factor().ok())
                    .unwrap_or(1.0);
                let Some(rect) = window_mgr::tray_bounds(&app, scale) else {
                    // 也取消尚在冷启动的 show intent，避免鼠标离开后才弹出。
                    window_mgr::hide_tooltip(&app);
                    inside_since = None;
                    continue;
                };
                let (x, y) = window_mgr::cursor_position();
                let inside = x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom;
                // 面板打开时仍允许悬停卡；预览/添加/右键菜单会挡住它。
                let blocked = window_visible(&app, window_mgr::POPUP)
                    || window_visible(&app, window_mgr::ADD)
                    || window_visible(&app, window_mgr::MENU);
                if inside && !blocked {
                    let since = *inside_since.get_or_insert_with(Instant::now);
                    let delay = state
                        .store
                        .lock()
                        .ok()
                        .map(|store| store.hover_delay())
                        .unwrap_or(0.3);
                    // closing 时重新 show 可取消前端正在播放的离场。
                    if since.elapsed().as_secs_f64() >= delay
                        && (!tooltip_visible || tooltip_closing)
                        && window_mgr::tooltip_can_show(&app, false, inside)
                    {
                        window_mgr::show_tooltip(&app, false);
                    }
                } else {
                    // 也取消尚在冷启动的 show intent，避免鼠标离开后才弹出。
                    window_mgr::hide_tooltip(&app);
                    inside_since = None;
                }
            }
        });
}

fn window_visible(app: &AppHandle, label: &str) -> bool {
    window_mgr::is_window_visible(app, label)
}
