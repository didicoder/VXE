use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::battery::{BatteryState, DeviceStatus};
use crate::config;
use crate::winutil;

pub const MAIN: &str = "main";
pub const POPUP: &str = "popup";
pub const TOOLTIP: &str = "tooltip";
pub const ADD: &str = "add";
pub const ALERT: &str = "alert";
pub const MENU: &str = "menu";
pub const INSPECTOR: &str = "inspector";
pub const BG: &str = "bg";

/// 卡片设置窗设计尺寸即 1×，当前固定不缩放；base 仅兼容旧配置/迁移。
const INSPECTOR_W: f64 = 400.0;
const INSPECTOR_H: f64 = 630.0;

/// 背景编辑器与卡片设置窗同规格。
const BG_W: f64 = 400.0;
const BG_H: f64 = 630.0;

fn inspector_scale(_store: &config::Store) -> f64 {
    1.0
}

fn bg_scale(_store: &config::Store) -> f64 {
    1.0
}

/// WebView2 用户数据目录：固定 exe 同级 `WebView2Data/`，不回退 AppData。
/// 这是浏览器运行时的可写 profile/缓存，无法塞进 exe；只读位置会启动失败。
pub fn webview_data_dir() -> Option<std::path::PathBuf> {
    let dir = config::program_dir().join("WebView2Data");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

/// 托盘右键菜单设计尺寸（窗口即卡面，不做外投影）：头部 34 + 3 项 37 + 分隔 + 内边距。
const MENU_W: f64 = 212.0;
const MENU_H: f64 = 170.0;

fn ui_scale(store: &config::Store) -> f64 {
    store.ui_scale()
}

fn popup_scale(store: &config::Store) -> f64 {
    store.popup_scale()
}

fn tip_scale(store: &config::Store) -> f64 {
    store.tip_scale()
}

fn alert_scale(store: &config::Store) -> f64 {
    store.alert_scale()
}

pub fn apply_main_geometry(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else {
        return;
    };
    let scale = app
        .try_state::<crate::commands::AppState>()
        .and_then(|state| state.store.lock().ok().map(|store| ui_scale(&store)))
        .unwrap_or(1.0);
    let _ = window.set_size(LogicalSize::new(1040.0 * scale, 720.0 * scale));
    let _ = window.center();
}

pub fn show_main(app: &AppHandle, page: &str) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("show-page", page);
    }
}

pub fn show_settings(app: &AppHandle) {
    show_main(app, "settings");
}

/// 预览小窗设计高（设计尺寸即 1×，当前只乘用户 popup_scale）：
/// base 仅兼容旧配置/迁移；头部 49.5 + Hero 102 + 分区 25.5 + 行区（含 6 底边距）
/// + 「还有 N 台」15 + 底栏间距 7.5 + 底栏 33 + 底部留白 12。
fn popup_design_height(count: usize) -> f64 {
    let rows = count.min(3);
    let more = usize::from(count > 3);
    let body = if rows == 0 {
        72.0
    } else {
        rows as f64 * 54.0 + rows.saturating_sub(1) as f64 * 6.0
    };
    49.5 + 102.0 + 25.5 + 6.0 + 7.5 + 33.0 + 12.0 + body + more as f64 * 15.0
}

pub fn show_popup(app: &AppHandle, hold: bool) -> bool {
    if let Some(state) = app.try_state::<crate::commands::AppState>() {
        state
            .popup_hold
            .store(hold, std::sync::atomic::Ordering::SeqCst);
    }
    let Some(window) = layout_popup(app) else {
        return false;
    };
    let shown = window.show().is_ok();
    if !hold {
        // 需要焦点才能靠失焦判断「点了卡片外」；hold 预览不抢焦点。
        let _ = window.set_focus();
    }
    let _ = app.emit("popup-open", hold);
    shown
}

/// 按当前缩放把预览小窗摆到托盘旁：只改尺寸/位置，不 show、不抢焦点、不发 `popup-open`。
fn layout_popup(app: &AppHandle) -> Option<WebviewWindow> {
    let scale = app
        .try_state::<crate::commands::AppState>()
        .and_then(|state| state.store.lock().ok().map(|store| popup_scale(&store)))
        .unwrap_or(1.0);
    let design_height = popup_design_height(status_count(app, "display_popup"));
    let width = 345.0 * scale;
    let height = design_height * scale;
    let window = ensure_window(app, POPUP, width, height, false, false).ok()?;
    let factor = window.scale_factor().unwrap_or(1.0);
    let physical_width = (width * factor).round() as i32;
    let physical_height = (height * factor).round() as i32;
    let gap = (7.5 * scale * factor).round() as i32;
    let (x, y) = anchor_near_tray(app, physical_width, physical_height, gap);
    let _ = window.set_position(PhysicalPosition::new(x, y));
    Some(window)
}

pub fn show_tooltip(app: &AppHandle, _hold: bool) {
    if let Some(state) = app.try_state::<crate::commands::AppState>() {
        state
            .tooltip_closing
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
    let Some(window) = layout_tooltip(app) else {
        return;
    };
    let _ = window.show();
    let _ = app.emit("tooltip-open", ());
}

/// 只按当前缩放和可见行数设置悬停卡尺寸/位置，不 show、不改 closing、不发 `tooltip-open`。
fn layout_tooltip(app: &AppHandle) -> Option<WebviewWindow> {
    let scale = app
        .try_state::<crate::commands::AppState>()
        .and_then(|state| state.store.lock().ok().map(|store| tip_scale(&store)))
        .unwrap_or(1.0);
    let rows = status_count(app, "display_tip").clamp(1, 6);
    let width = 243.1 * scale;
    let height = (35.7 + rows as f64 * 42.5 + 6.8) * scale;
    let window = ensure_window(app, TOOLTIP, width, height, true, true).ok()?;
    let factor = window.scale_factor().unwrap_or(1.0);
    let physical_width = (width * factor).round() as i32;
    let physical_height = (height * factor).round() as i32;
    let gap = (6.8 * scale * factor).round() as i32;
    let (x, y) = anchor_near_tray(app, physical_width, physical_height, gap);
    let _ = window.set_position(PhysicalPosition::new(x, y));
    Some(window)
}

/// 悬停卡离场由前端播完 140ms 再 hide；这里只发一次 close，避免 hover 线程反复发。
pub fn hide_tooltip(app: &AppHandle) {
    let Some(state) = app.try_state::<crate::commands::AppState>() else {
        return;
    };
    if !window_visible(app, TOOLTIP) {
        state
            .tooltip_closing
            .store(false, std::sync::atomic::Ordering::SeqCst);
        return;
    }
    if !state
        .tooltip_closing
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        let _ = app.emit("tooltip-close", ());
    }
}

/// 自定义托盘右键菜单：按托盘锚点定位；打开时与悬停卡/预览互斥。
pub fn show_tray_menu(app: &AppHandle) {
    hide(app, TOOLTIP);
    hide_popup(app);
    let width = MENU_W;
    let height = MENU_H;
    let Ok(window) = ensure_window(app, MENU, width, height, false, false) else {
        return;
    };
    let factor = window.scale_factor().unwrap_or(1.0);
    let physical_width = (width * factor).round() as i32;
    let physical_height = (height * factor).round() as i32;
    let gap = (2.0 * factor).round() as i32;
    let (x, y) = anchor_near_tray(app, physical_width, physical_height, gap);
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit("tray-menu-open", ());
}

/// 打开卡片设置窗：首次贴主窗右侧（空间不足则左侧），之后保留用户拖动的位置。
pub fn show_inspector(app: &AppHandle) -> Result<(), String> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if let Err(err) = show_inspector_inner(&handle) {
            eprintln!("[show_inspector] {err}");
        }
    })
    .map_err(|err| err.to_string())
}

fn show_inspector_inner(app: &AppHandle) -> Result<(), String> {
    let scale = app
        .try_state::<crate::commands::AppState>()
        .and_then(|state| state.store.lock().ok().map(|store| inspector_scale(&store)))
        .unwrap_or(1.0);
    let window = ensure_window(
        app,
        INSPECTOR,
        INSPECTOR_W * scale,
        INSPECTOR_H * scale,
        false,
        false,
    )
    .map_err(|err| err.to_string())?;
    let first = app
        .try_state::<crate::commands::AppState>()
        .map(|state| {
            !state
                .inspector_placed
                .swap(true, std::sync::atomic::Ordering::SeqCst)
        })
        .unwrap_or(true);
    if first {
        let factor = window.scale_factor().unwrap_or(1.0);
        let width = (INSPECTOR_W * scale * factor).round() as i32;
        let height = (INSPECTOR_H * scale * factor).round() as i32;
        place_beside_main(app, &window, width, height);
    }
    window.show().map_err(|err| err.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

fn place_beside_main(app: &AppHandle, window: &WebviewWindow, width: i32, height: i32) {
    let Some(main) = app.get_webview_window(MAIN) else {
        return;
    };
    let (Ok(position), Ok(size)) = (main.outer_position(), main.outer_size()) else {
        return;
    };
    let cx = position.x + size.width as i32 / 2;
    let cy = position.y + size.height as i32 / 2;
    let Some(work) = winutil::work_area_for_point(cx, cy) else {
        return;
    };
    let gap = 12;
    let mut x = position.x + size.width as i32 + gap;
    let mut y = position.y + (size.height as i32 - height) / 2;
    if x + width > work.right {
        x = position.x - width - gap;
    }
    let max_x = (work.right - width - 8).max(work.left + 8);
    let max_y = (work.bottom - height - 8).max(work.top + 8);
    x = x.clamp(work.left + 8, max_x);
    y = y.clamp(work.top + 8, max_y);
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

pub fn hide_inspector(app: &AppHandle) {
    hide(app, INSPECTOR);
    let _ = app.emit("inspector-close", ());
}

pub fn show_bg_editor(app: &AppHandle) -> Result<(), String> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if let Err(err) = show_bg_editor_inner(&handle) {
            eprintln!("[show_bg_editor] {err}");
        }
    })
    .map_err(|err| err.to_string())
}

fn show_bg_editor_inner(app: &AppHandle) -> Result<(), String> {
    let scale = app
        .try_state::<crate::commands::AppState>()
        .and_then(|state| state.store.lock().ok().map(|store| bg_scale(&store)))
        .unwrap_or(1.0);
    let window = ensure_window(app, BG, BG_W * scale, BG_H * scale, false, false)
        .map_err(|err| err.to_string())?;
    let first = app
        .try_state::<crate::commands::AppState>()
        .map(|state| {
            !state
                .bg_editor_placed
                .swap(true, std::sync::atomic::Ordering::SeqCst)
        })
        .unwrap_or(true);
    if first {
        let factor = window.scale_factor().unwrap_or(1.0);
        let width = (BG_W * scale * factor).round() as i32;
        let height = (BG_H * scale * factor).round() as i32;
        place_beside_main(app, &window, width, height);
    }
    window.show().map_err(|err| err.to_string())?;
    let _ = window.set_focus();
    let _ = app.emit("bg-editor-open", ());
    Ok(())
}

pub fn hide_bg_editor(app: &AppHandle) {
    hide(app, BG);
    let _ = app.emit("bg-editor-close", ());
}

pub fn show_add(app: &AppHandle) -> Result<(), String> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if let Err(err) = show_add_inner(&handle) {
            eprintln!("[show_add] {err}");
        }
    })
    .map_err(|err| err.to_string())
}

fn show_add_inner(app: &AppHandle) -> Result<(), String> {
    let window = ensure_window(app, ADD, 459.0, 586.5, false, false).map_err(|err| {
        eprintln!("[show_add] ensure failed: {err}");
        err.to_string()
    })?;
    let _ = window.center();
    window.show().map_err(|err| {
        eprintln!("[show_add] show failed: {err}");
        err.to_string()
    })?;
    let _ = window.set_focus();
    let _ = app.emit("add-open", ());
    Ok(())
}

pub fn show_alert(app: &AppHandle) {
    let scale = app
        .try_state::<crate::commands::AppState>()
        .and_then(|state| state.store.lock().ok().map(|store| alert_scale(&store)))
        .unwrap_or(1.0);
    let width = 696.0 * scale;
    let height = 160.0 * scale;
    let Ok(window) = ensure_window(app, ALERT, width, height, true, true) else {
        return;
    };
    let factor = window.scale_factor().unwrap_or(1.0);
    let physical_width = (width * factor).round() as i32;
    let physical_height = (height * factor).round() as i32;
    set_alert_position_mode(app, false);
    let (sw, sh) = winutil::primary_screen_size();
    let work = winutil::work_area_for_point(sw / 2, sh / 2).unwrap_or(winutil::Rect {
        left: 0,
        top: 0,
        right: sw,
        bottom: sh,
    });
    let saved = app
        .try_state::<crate::commands::AppState>()
        .and_then(|state| state.store.lock().ok().and_then(|store| store.alert_pos()));
    let (x, y) = if let Some(saved) = saved {
        saved_alert_position(saved, physical_width, physical_height, work)
    } else {
        // 胶囊中心距工作区底部 28 + 32（半个胶囊），窗口含 48 px 光晕留白，
        // 因此窗口底边会越过工作区 20 px（与 V16 `_place` 一致）。
        (
            work.left + (work.width() - physical_width) / 2,
            work.bottom - physical_height + (20.0 * scale * factor).round() as i32,
        )
    };
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = app.emit("alert-open", ());
}

fn saved_alert_position(
    (cx, cy): (i32, i32),
    width: i32,
    height: i32,
    work: winutil::Rect,
) -> (i32, i32) {
    let max_x = (work.right - width).max(work.left);
    let max_y = (work.bottom - height).max(work.top);
    (
        (cx - width / 2).clamp(work.left, max_x),
        (cy - height / 2).clamp(work.top, max_y),
    )
}

pub fn set_alert_position_mode(app: &AppHandle, enabled: bool) {
    let Some(window) = app.get_webview_window(ALERT) else {
        return;
    };
    let _ = window.set_ignore_cursor_events(!enabled);
    let _ = set_utility_styles(&window, !enabled, !enabled);
    if enabled {
        let _ = window.set_focus();
    }
}

pub fn prewarm(app: &AppHandle) {
    let (popup, tip, alert) = app
        .try_state::<crate::commands::AppState>()
        .and_then(|state| {
            state
                .store
                .lock()
                .ok()
                .map(|store| (store.popup_scale(), store.tip_scale(), store.alert_scale()))
        })
        .unwrap_or((1.0, 1.0, 1.0));
    if let Err(err) = ensure_window(app, POPUP, 345.0 * popup, 409.5 * popup, false, false) {
        eprintln!("[window] prewarm popup failed: {err}");
    }
    if let Err(err) = ensure_window(app, TOOLTIP, 243.1 * tip, 85.0 * tip, true, true) {
        eprintln!("[window] prewarm tooltip failed: {err}");
    }
    if let Err(err) = ensure_window(app, ADD, 459.0, 586.5, false, false) {
        eprintln!("[window] prewarm add failed: {err}");
    }
    let inspector = 1.0;
    if let Err(err) = ensure_window(
        app,
        INSPECTOR,
        INSPECTOR_W * inspector,
        INSPECTOR_H * inspector,
        false,
        false,
    ) {
        eprintln!("[window] prewarm inspector failed: {err}");
    }
    if let Err(err) = ensure_window(app, ALERT, 696.0 * alert, 160.0 * alert, true, true) {
        eprintln!("[window] prewarm alert failed: {err}");
    }
    if let Err(err) = ensure_window(app, MENU, MENU_W, MENU_H, false, false) {
        eprintln!("[window] prewarm menu failed: {err}");
    }
    let bg = 1.0;
    if let Err(err) = ensure_window(app, BG, BG_W * bg, BG_H * bg, false, false) {
        eprintln!("[window] prewarm bg failed: {err}");
    }
}

pub fn hide(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
    }
}

/// Rust 侧直接收起预览小窗时通知前端复位（否则下一轮 popup-open 会误判仍在打开）。
pub fn hide_popup(app: &AppHandle) {
    hide(app, POPUP);
    let _ = app.emit("popup-close", ());
}

fn window_visible(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

/// 缩放、设备刷新或显示设置变化后，按当前可见行数重排已显示的预览小窗。
/// 重排不是「打开」：不发 `popup-open`，否则前端会再请求一次整页刷新。
pub fn relayout_popup(app: &AppHandle) {
    if !window_visible(app, POPUP) {
        return;
    }
    let _ = layout_popup(app);
}

#[derive(Debug, PartialEq, Eq)]
enum TooltipRelayoutAction {
    Skip,
    Layout,
}

fn tooltip_relayout_action(visible: bool) -> TooltipRelayoutAction {
    if visible {
        TooltipRelayoutAction::Layout
    } else {
        TooltipRelayoutAction::Skip
    }
}

/// 缩放、设备刷新或显示设置变化后，按当前可见行数重排已显示的悬停卡。
pub fn relayout_tooltip(app: &AppHandle) {
    match tooltip_relayout_action(window_visible(app, TOOLTIP)) {
        TooltipRelayoutAction::Skip => {}
        TooltipRelayoutAction::Layout => {
            let _ = layout_tooltip(app);
        }
    }
}

fn status_count(app: &AppHandle, display_key: &str) -> usize {
    app.try_state::<crate::commands::AppState>()
        .and_then(|state| {
            let display = state
                .store
                .lock()
                .ok()?
                .get(display_key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("all")
                .to_owned();
            state
                .statuses
                .lock()
                .ok()
                .map(|statuses| visible_status_count(&statuses, &display))
        })
        .unwrap_or(0)
}

fn visible_status_count(statuses: &[DeviceStatus], display: &str) -> usize {
    statuses
        .iter()
        .filter(|status| display == "all" || status.state != BatteryState::Absent)
        .count()
}

fn ensure_window(
    app: &AppHandle,
    label: &str,
    width: f64,
    height: f64,
    passive: bool,
    no_activate: bool,
) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_size(LogicalSize::new(width, height));
        return Ok(window);
    }
    // 托盘弹层/悬停/提醒保持置顶；添加设备、卡片设置与背景编辑器是常规窗口，不置顶。
    let normal = matches!(label, ADD | INSPECTOR | BG);
    let topmost = !normal;
    let title = match label {
        INSPECTOR => "卡片设置",
        BG => "自定义背景",
        _ => "VXE",
    };
    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(width, height)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(topmost)
        .skip_taskbar(!normal)
        .resizable(false)
        .visible(false)
        .focused(false);
    if let Some(dir) = webview_data_dir() {
        builder = builder.data_directory(dir);
    }
    let window = builder.build()?;
    if let Err(err) = set_utility_styles(&window, passive, no_activate) {
        eprintln!("[window] {label} style: {err}");
    }
    if passive {
        let _ = window.set_ignore_cursor_events(true);
    }
    Ok(window)
}

fn set_utility_styles(
    window: &WebviewWindow,
    passive: bool,
    no_activate: bool,
) -> tauri::Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
            WS_EX_TRANSPARENT,
        };
        let hwnd = window.hwnd()?;
        let mut style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        style &= !(WS_EX_NOACTIVATE.0 as isize
            | WS_EX_TOOLWINDOW.0 as isize
            | WS_EX_TRANSPARENT.0 as isize);
        if no_activate {
            style |= WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize;
        }
        if passive {
            style |= WS_EX_TRANSPARENT.0 as isize;
        }
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        }
    }
    #[cfg(not(windows))]
    let _ = (window, passive, no_activate);
    Ok(())
}

fn anchor_near_tray(app: &AppHandle, width: i32, height: i32, gap: i32) -> (i32, i32) {
    let scale = app
        .get_webview_window(MAIN)
        .and_then(|window| window.scale_factor().ok())
        .unwrap_or(1.0);
    if let Some(rect) = tray_rect(app, scale)
        && let Some((x, y)) = place_relative(rect, width, height, gap)
    {
        return (x, y);
    }
    let (cx, cy) = cursor_position();
    let rect = winutil::Rect {
        left: cx,
        top: cy,
        right: cx + 1,
        bottom: cy + 1,
    };
    place_relative(rect, width, height, gap).unwrap_or((cx, cy))
}

fn place_relative(rect: winutil::Rect, width: i32, height: i32, gap: i32) -> Option<(i32, i32)> {
    let (sw, sh) = winutil::primary_screen_size();
    let work =
        winutil::work_area_for_point((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2)
            .unwrap_or(winutil::Rect {
                left: 0,
                top: 0,
                right: sw,
                bottom: sh,
            });
    Some(winutil::anchor_for_rect(
        rect, width, height, gap, sw, sh, work,
    ))
}

pub fn tray_bounds(app: &AppHandle, scale: f64) -> Option<winutil::Rect> {
    tray_rect(app, scale)
}

fn tray_rect(app: &AppHandle, scale: f64) -> Option<winutil::Rect> {
    if crate::tray::is_promoted() == Some(false) {
        return None;
    }
    let tray = app.try_state::<tauri::tray::TrayIcon<tauri::Wry>>()?;
    let rect = tray.rect().ok().flatten()?;
    let (x, y) = match rect.position {
        tauri::Position::Physical(position) => (position.x, position.y),
        tauri::Position::Logical(position) => (
            (position.x * scale).round() as i32,
            (position.y * scale).round() as i32,
        ),
    };
    let (w, h) = match rect.size {
        tauri::Size::Physical(size) => (size.width as i32, size.height as i32),
        tauri::Size::Logical(size) => (
            (size.width * scale).round() as i32,
            (size.height * scale).round() as i32,
        ),
    };
    Some(winutil::Rect {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    })
}

pub fn cursor_position() -> (i32, i32) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut point = POINT::default();
        // SAFETY: point is a valid out-parameter for GetCursorPos.
        if unsafe { GetCursorPos(&mut point) }.is_ok() {
            return (point.x, point.y);
        }
    }
    (0, 0)
}
