use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
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

const REAP_AFTER: Duration = Duration::from_secs(5);
const REAP_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum OverlayMode {
    #[default]
    Popup,
    Tooltip,
}

impl OverlayMode {
    fn from_label(label: &str) -> Option<Self> {
        match label {
            POPUP => Some(Self::Popup),
            TOOLTIP => Some(Self::Tooltip),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Popup => POPUP,
            Self::Tooltip => TOOLTIP,
        }
    }

    fn owns(self, label: &str) -> bool {
        match Self::from_label(label) {
            Some(mode) => mode == self,
            None => true,
        }
    }
}

fn physical_label(label: &str) -> &str {
    if OverlayMode::from_label(label).is_some() {
        POPUP
    } else {
        label
    }
}

fn logical_window_visible(mode: OverlayMode, label: &str, physical_visible: bool) -> bool {
    physical_visible && mode.owns(label)
}

fn insert_intent(intents: &mut HashMap<String, Intent>, label: &str, intent: Intent) {
    let physical = physical_label(label);
    if matches!(
        intent,
        Intent::Show {
            label: TOOLTIP,
            event: Some((_, Some(false))),
            ..
        }
    ) && intents.get(physical).and_then(intent_mode) == Some(OverlayMode::Popup)
    {
        return;
    }
    intents.insert(physical.to_owned(), intent);
}

fn intent_mode(intent: &Intent) -> Option<OverlayMode> {
    match intent {
        Intent::Show { label, .. } => OverlayMode::from_label(label),
        Intent::Main { .. } | Intent::Alert { .. } => None,
    }
}

fn cancel_intent(intents: &mut HashMap<String, Intent>, label: &str) {
    let physical = physical_label(label);
    let matches_mode = intents
        .get(physical)
        .and_then(intent_mode)
        .is_some_and(|mode| Some(mode) == OverlayMode::from_label(label));
    if matches_mode || OverlayMode::from_label(label).is_none() {
        intents.remove(physical);
    }
}

fn should_emit_open(label: &str, was_visible: bool) -> bool {
    !was_visible || OverlayMode::from_label(label).is_some()
}

fn request_overlay_mode(app: &AppHandle, label: &str) {
    if let Some(mode) = OverlayMode::from_label(label)
        && let Some(runtime) = app.try_state::<Runtime>()
    {
        runtime.request_overlay_mode(mode.label());
    }
}

fn intent_should_show(intent: &Intent, cursor_over: bool) -> bool {
    match intent {
        Intent::Show {
            label: TOOLTIP,
            event: Some((_, Some(hold))),
            ..
        } => tooltip_should_show(*hold, cursor_over),
        Intent::Show { label: TOOLTIP, .. } => cursor_over,
        _ => true,
    }
}

#[derive(Clone, Debug)]
enum Intent {
    Show {
        label: &'static str,
        event: Option<(&'static str, Option<bool>)>,
        focus: bool,
    },
    Main {
        page: String,
    },
    Alert {
        event: String,
        payload: serde_json::Value,
    },
}

type DeferredAction<C> = Box<dyn FnOnce(&C) + Send>;

struct ActionQueue<C> {
    pending: Mutex<VecDeque<DeferredAction<C>>>,
    draining: AtomicBool,
}

impl<C> Default for ActionQueue<C> {
    fn default() -> Self {
        Self {
            pending: Mutex::new(VecDeque::new()),
            draining: AtomicBool::new(false),
        }
    }
}

impl<C> ActionQueue<C> {
    fn push(&self, action: impl FnOnce(&C) + Send + 'static) -> Result<(), String> {
        self.pending
            .lock()
            .map_err(|_| "window action lock poisoned")?
            .push_back(Box::new(action));
        Ok(())
    }

    fn drain(&self, context: &C) {
        // WebView2 creation pumps Windows messages. Nested event-loop callbacks
        // must not start another lifecycle operation during build/destroy.
        if self.draining.swap(true, Ordering::SeqCst) {
            return;
        }
        let actions = self
            .pending
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue));
        if let Ok(actions) = actions {
            for action in actions {
                action(context);
            }
        }
        self.draining.store(false, Ordering::SeqCst);
    }
}

#[derive(Default)]
pub struct Runtime {
    actions: ActionQueue<AppHandle>,
    tray_preview_requested: AtomicBool,
    ready: Mutex<std::collections::HashSet<String>>,
    pending: Mutex<HashMap<String, Intent>>,
    hidden_since: Mutex<HashMap<String, Instant>>,
    main_page: Mutex<String>,
    positions: Mutex<HashMap<String, PhysicalPosition<i32>>>,
    destroying: Mutex<std::collections::HashSet<String>>,
    deferred: Mutex<HashMap<String, Intent>>,
    alert_visible: Mutex<bool>,
    alert_shown: Condvar,
    overlay_mode: Mutex<OverlayMode>,
    active_overlay_mode: Mutex<Option<OverlayMode>>,
}

impl Runtime {
    fn tooltip_can_show(&self, hold: bool, cursor_over: bool) -> bool {
        !self.tray_preview_requested.load(Ordering::SeqCst)
            && tooltip_should_show(hold, cursor_over)
    }

    fn request_tray_preview(&self) {
        self.tray_preview_requested.store(true, Ordering::SeqCst);
    }

    fn release_tray_preview(&self) {
        self.tray_preview_requested.store(false, Ordering::SeqCst);
    }

    fn request_overlay_mode(&self, label: &str) -> bool {
        let Some(mode) = OverlayMode::from_label(label) else {
            return false;
        };
        let Ok(mut current) = self.overlay_mode.lock() else {
            return false;
        };
        let changed = *current != mode;
        *current = mode;
        changed
    }

    fn overlay_mode(&self) -> OverlayMode {
        self.overlay_mode
            .lock()
            .map(|mode| *mode)
            .unwrap_or_default()
    }

    fn active_overlay_mode(&self) -> Option<OverlayMode> {
        self.active_overlay_mode
            .lock()
            .map(|mode| *mode)
            .unwrap_or(None)
    }

    fn prepare_hide(&self, label: &str) -> bool {
        // Logical modes share a physical slot; cancel only this mode before
        // deciding whether the currently active window can be hidden.
        if let Ok(mut pending) = self.pending.lock() {
            cancel_intent(&mut pending, label);
        }
        if let Ok(mut deferred) = self.deferred.lock() {
            cancel_intent(&mut deferred, label);
        }
        if label == POPUP {
            self.release_tray_preview();
        }
        match OverlayMode::from_label(label) {
            Some(mode) => self.active_overlay_mode() == Some(mode),
            None => true,
        }
    }
}

fn on_main(
    app: &AppHandle,
    action: impl FnOnce(&AppHandle) + Send + 'static,
) -> Result<(), String> {
    let runtime = app
        .try_state::<Runtime>()
        .ok_or("window runtime is not managed")?;
    runtime.actions.push(action)?;
    // run_on_main_thread runs inline on the UI thread. Only use it to wake the
    // event loop; building inside a WebView IPC callback deadlocks on Windows.
    app.run_on_main_thread(|| {}).map_err(|err| err.to_string())
}

/// Called after input/IPC callbacks have returned, never from a WebView callback.
pub fn drain_actions(app: &AppHandle) {
    let Some(runtime) = app.try_state::<Runtime>() else {
        return;
    };
    runtime.actions.drain(app);
}

fn set_inactive(window: &WebviewWindow, inactive: bool) {
    let script = if inactive {
        "document.documentElement.classList.add('window-inactive')"
    } else {
        "document.documentElement.classList.remove('window-inactive')"
    };
    let _ = window.eval(script);
}

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
    let _ = on_main(app, apply_main_geometry_inner);
}

fn apply_main_geometry_inner(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else {
        return;
    };
    let scale = app
        .try_state::<crate::commands::AppState>()
        .and_then(|state| state.store.lock().ok().map(|store| ui_scale(&store)))
        .unwrap_or(1.0);
    let _ = window.set_size(LogicalSize::new(832.0 * scale, 576.0 * scale));
    if !restore_position(app, &window) {
        let _ = window.center();
    }
}

pub fn show_main(app: &AppHandle, page: &str) {
    crate::tray::tray_log("main requested");
    let page = page.to_owned();
    let _ = on_main(app, move |app| show_main_inner(app, &page));
}

fn show_main_inner(app: &AppHandle, page: &str) {
    let page = if page.is_empty() {
        app.try_state::<Runtime>()
            .and_then(|runtime| runtime.main_page.lock().ok().map(|page| page.clone()))
            .filter(|page| !page.is_empty())
            .unwrap_or_else(|| "summary".into())
    } else {
        page.to_owned()
    };
    let intent = Intent::Main { page: page.clone() };
    if defer_destroying(app, MAIN, &intent) {
        return;
    }
    if let Err(err) = ensure_main(app) {
        eprintln!("[show_main] {err}");
        return;
    }
    remember_main_page(app, &page);
    request_show(app, MAIN, intent);
}

fn ensure_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(MAIN) {
        return Ok(window);
    }
    let conf = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN)
        .ok_or_else(|| tauri::Error::InvalidWebviewUrl("missing main window config"))?;
    let browser_args =
        std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
    let mut builder = WebviewWindowBuilder::from_config(app, conf)?
        .additional_browser_args(&crate::browser_arguments(&browser_args))
        .visible(false)
        .focused(false);
    if let Some(dir) = webview_data_dir() {
        builder = builder.data_directory(dir);
    }
    crate::tray::tray_log("main build start");
    let window = builder.build().inspect_err(|err| {
        crate::tray::tray_log(&format!("main build failed: {err}"));
    })?;
    crate::tray::tray_log("main build complete");
    register_created(app, &window);
    apply_main_geometry_inner(app);
    if !restore_position(app, &window) {
        let _ = window.center();
    }
    crate::theme::apply(app);
    crate::theme::apply_icon_for_current(app);
    Ok(window)
}

fn defer_destroying(app: &AppHandle, label: &str, intent: &Intent) -> bool {
    let Some(runtime) = app.try_state::<Runtime>() else {
        return false;
    };
    let physical = physical_label(label);
    let destroying = runtime
        .destroying
        .lock()
        .ok()
        .map(|set| set.contains(physical))
        .unwrap_or(false);
    if destroying && let Ok(mut deferred) = runtime.deferred.lock() {
        insert_intent(&mut deferred, label, intent.clone());
    }
    destroying
}

fn restore_position(app: &AppHandle, window: &WebviewWindow) -> bool {
    let saved = app.try_state::<Runtime>().and_then(|runtime| {
        runtime
            .positions
            .lock()
            .ok()
            .and_then(|map| map.get(window.label()).copied())
    });
    if let Some(position) = saved {
        let _ = window.set_position(position);
        true
    } else {
        false
    }
}

fn register_created(app: &AppHandle, window: &WebviewWindow) {
    let handle = app.clone();
    let label = window.label().to_owned();
    let physical = physical_label(&label).to_owned();
    window.on_window_event(move |event| {
        if !matches!(event, tauri::WindowEvent::Destroyed) {
            return;
        }
        crate::tray::tray_log(&format!("{label} destroyed"));
        let Some(runtime) = handle.try_state::<Runtime>() else {
            return;
        };
        if let Ok(mut ready) = runtime.ready.lock() {
            ready.remove(&physical);
        }
        if let Ok(mut pending) = runtime.pending.lock() {
            pending.remove(&physical);
        }
        if physical == POPUP
            && let Ok(mut active) = runtime.active_overlay_mode.lock()
        {
            *active = None;
        }
        if label == ALERT
            && let Ok(mut visible) = runtime.alert_visible.lock()
        {
            *visible = false;
        }
        // 直到 Manager 不再返回旧句柄，回收轮询才解除 destroying 并重建。
    });
}

fn reopen_intent(app: &AppHandle, intent: Intent) {
    match intent {
        Intent::Main { page } => show_main_inner(app, &page),
        Intent::Alert { event, payload } => show_alert_event_inner(app, event, payload),
        Intent::Show {
            label: POPUP,
            event,
            ..
        } => {
            show_popup_inner(app, event.and_then(|(_, hold)| hold).unwrap_or(false));
        }
        Intent::Show {
            label: TOOLTIP,
            event,
            ..
        } => show_tooltip_inner(app, event.and_then(|(_, hold)| hold).unwrap_or(false)),
        Intent::Show { label: MENU, .. } => show_tray_menu_inner(app),
        Intent::Show { label: ADD, .. } => {
            let _ = show_add_inner(app);
        }
        Intent::Show {
            label: INSPECTOR, ..
        } => {
            let _ = show_inspector_inner(app);
        }
        Intent::Show { label: BG, .. } => {
            let _ = show_bg_editor_inner(app);
        }
        _ => {}
    }
}

fn request_show(app: &AppHandle, label: &'static str, intent: Intent) {
    request_overlay_mode(app, label);
    if defer_destroying(app, label, &intent) {
        return;
    }
    let physical = physical_label(label);
    let ready = app
        .try_state::<Runtime>()
        .and_then(|runtime| runtime.ready.lock().ok().map(|set| set.contains(physical)))
        .unwrap_or(false);
    if !ready {
        if let Some(runtime) = app.try_state::<Runtime>()
            && let Ok(mut pending) = runtime.pending.lock()
        {
            insert_intent(&mut pending, label, intent);
        }
        return;
    }
    apply_intent(app, intent);
}

fn apply_intent(app: &AppHandle, intent: Intent) {
    if let Intent::Alert { event, payload } = &intent {
        let Some(window) = app.get_webview_window(ALERT) else {
            return;
        };
        set_inactive(&window, false);
        set_alert_position_mode_inner(app, false);
        if window.show().is_ok() {
            let _ = window.emit(event, payload);
            if event == "alert-position-start" {
                set_alert_position_mode_window(&window, true);
            }
            if let Some(runtime) = app.try_state::<Runtime>() {
                if let Ok(mut visible) = runtime.alert_visible.lock() {
                    *visible = true;
                    runtime.alert_shown.notify_all();
                }
                if let Ok(mut hidden) = runtime.hidden_since.lock() {
                    hidden.remove(ALERT);
                }
            }
        }
        return;
    }
    let (label, page, event, focus) = match intent {
        Intent::Main { page } => (MAIN, Some(page), None, true),
        Intent::Alert { .. } => unreachable!(),
        Intent::Show {
            label,
            event,
            focus,
        } => (label, None, event, focus),
    };
    let physical = physical_label(label);
    let Some(window) = app.get_webview_window(physical) else {
        return;
    };
    let was_visible = window.is_visible().unwrap_or(false);
    set_inactive(&window, false);
    if let Some(mode) = OverlayMode::from_label(label) {
        let _ = window.emit("tray-overlay-mode", mode.label());
        if let Some(runtime) = app.try_state::<Runtime>()
            && let Ok(mut active) = runtime.active_overlay_mode.lock()
        {
            *active = Some(mode);
        }
    }
    if label == MAIN && window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    if let Err(err) = window.show() {
        crate::tray::tray_log(&format!("{label} show failed: {err}"));
        if label == POPUP
            && let Some(runtime) = app.try_state::<Runtime>()
        {
            runtime.release_tray_preview();
        }
        return;
    }
    crate::tray::tray_log(&format!("{label} shown"));
    if label == POPUP
        && let Some(runtime) = app.try_state::<Runtime>()
    {
        runtime.release_tray_preview();
    }
    if focus {
        let _ = window.set_focus();
    }
    if let Some(runtime) = app.try_state::<Runtime>()
        && let Ok(mut hidden) = runtime.hidden_since.lock()
    {
        hidden.remove(physical);
    }
    if let Some(page) = page {
        let _ = window.emit("show-page", page);
    }
    if label == INSPECTOR {
        if let Some(target) = app
            .try_state::<crate::commands::AppState>()
            .and_then(|state| {
                state
                    .inspector_target
                    .lock()
                    .ok()
                    .and_then(|target| target.clone())
            })
        {
            let _ = window.emit("inspector-open", target);
        }
        return;
    }
    if should_emit_open(label, was_visible)
        && let Some((name, payload)) = event
    {
        match payload {
            Some(value) => {
                let _ = window.emit(name, value);
            }
            None => {
                let _ = window.emit(name, ());
            }
        }
    }
}

pub fn on_ready(window: &WebviewWindow) -> Result<(), String> {
    let window = window.clone();
    let app = window.app_handle().clone();
    on_main(&app, move |_| {
        if let Err(err) = on_ready_inner(&window) {
            eprintln!("[window_ready] {err}");
        }
    })
}

fn on_ready_inner(window: &WebviewWindow) -> Result<(), String> {
    let app = window.app_handle();
    let label = physical_label(window.label()).to_owned();
    crate::tray::tray_log(&format!("{label} frontend ready"));
    let runtime = app
        .try_state::<Runtime>()
        .ok_or("window runtime is not managed")?;
    let first_ready = runtime
        .ready
        .lock()
        .map_err(|_| "window readiness lock poisoned")?
        .insert(label.clone());
    if !first_ready {
        return Ok(());
    }
    set_inactive(window, true);
    let intent = runtime
        .pending
        .lock()
        .map_err(|_| "window intent lock poisoned")?
        .remove(&label);
    if let Some(intent) = intent {
        if label == MAIN
            && let Intent::Main { page } = &intent
        {
            remember_main_page(app, page);
        }
        if !intent_should_show(&intent, cursor_over_tray(app))
            || (intent_mode(&intent) == Some(OverlayMode::Tooltip)
                && !runtime.tooltip_can_show(true, true))
        {
            return Ok(());
        }
        apply_intent(app, intent);
    }
    Ok(())
}

pub fn start_reaper(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("vxe-window-reaper".into())
        .spawn(move || {
            loop {
                std::thread::sleep(REAP_INTERVAL);
                reap_hidden(&app);
            }
        });
}

fn tooltip_should_show(hold: bool, cursor_over: bool) -> bool {
    hold || cursor_over
}

pub(crate) fn tooltip_can_show(app: &AppHandle, hold: bool, cursor_over: bool) -> bool {
    app.try_state::<Runtime>()
        .is_some_and(|runtime| runtime.tooltip_can_show(hold, cursor_over))
}

pub(crate) fn cursor_over_tray(app: &AppHandle) -> bool {
    let Some(rect) = tray_rect(app, 1.0) else {
        return true;
    };
    let (x, y) = cursor_position();
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

fn is_reap_candidate(visible: bool, protected: bool, pending: bool, ready: bool) -> bool {
    !visible && !protected && !pending && ready
}

fn reap_hidden(app: &AppHandle) {
    let _ = on_main(app, reap_hidden_on_main);
}

fn reap_hidden_on_main(app: &AppHandle) {
    let Some(runtime) = app.try_state::<Runtime>() else {
        return;
    };
    let labels = [MAIN, POPUP, ADD, INSPECTOR, ALERT, MENU, BG];
    let now = Instant::now();
    for label in labels {
        let Some(window) = app.get_webview_window(label) else {
            if let Ok(mut destroying) = runtime.destroying.lock() {
                destroying.remove(label);
            }
            let deferred = runtime
                .deferred
                .lock()
                .ok()
                .and_then(|mut map| map.remove(label));
            if let Some(intent) = deferred {
                reopen_intent(app, intent);
            }
            continue;
        };
        if runtime
            .destroying
            .lock()
            .ok()
            .map(|set| set.contains(label))
            .unwrap_or(false)
        {
            continue;
        }
        let minimized = window.is_minimized().unwrap_or(false);
        let visible = window.is_visible().unwrap_or(false);
        let protected = label == ALERT
            && app
                .try_state::<crate::commands::AppState>()
                .map(|state| {
                    state
                        .alert_showing
                        .load(std::sync::atomic::Ordering::SeqCst)
                })
                .unwrap_or(false);
        let pending = runtime
            .pending
            .lock()
            .ok()
            .map(|map| map.contains_key(label))
            .unwrap_or(false);
        let ready = runtime
            .ready
            .lock()
            .ok()
            .map(|set| set.contains(label))
            .unwrap_or(false);
        if ready {
            set_inactive(&window, !visible || minimized);
        }
        if !is_reap_candidate(visible || minimized, protected, pending, ready) {
            runtime
                .hidden_since
                .lock()
                .ok()
                .map(|mut map| map.remove(label));
            continue;
        }
        let expired = runtime
            .hidden_since
            .lock()
            .ok()
            .map(|mut map| {
                let since = map.entry(label.to_owned()).or_insert(now);
                now.duration_since(*since) >= REAP_AFTER
            })
            .unwrap_or(false);
        if expired {
            let Ok(mut destroying) = runtime.destroying.lock() else {
                continue;
            };
            if !destroying.insert(label.to_owned()) {
                continue;
            }
            drop(destroying);
            if let Ok(position) = window.outer_position()
                && let Ok(mut positions) = runtime.positions.lock()
            {
                positions.insert(label.to_owned(), position);
            }
            set_inactive(&window, true);
            if window.destroy().is_err()
                && let Ok(mut destroying) = runtime.destroying.lock()
            {
                destroying.remove(label);
            }
            if let Ok(mut hidden) = runtime.hidden_since.lock() {
                hidden.remove(label);
            }
        }
    }
}

/// Read-only startup hint; never creates a window inside an IPC callback.
pub fn main_page(app: &AppHandle) -> String {
    app.try_state::<Runtime>()
        .and_then(|runtime| runtime.main_page.lock().ok().map(|page| page.clone()))
        .filter(|page| matches!(page.as_str(), "summary" | "devices" | "settings"))
        .unwrap_or_else(|| "summary".to_owned())
}

pub fn remember_main_page(app: &AppHandle, page: &str) {
    if let Some(runtime) = app.try_state::<Runtime>()
        && let Ok(mut current) = runtime.main_page.lock()
    {
        *current = page.to_owned();
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

/// Returns whether the request was queued; creation failures fall back inside
/// the queued action, after the tray callback has returned.
pub fn show_popup(app: &AppHandle, hold: bool) -> bool {
    let runtime = app.try_state::<Runtime>();
    if !hold && let Some(runtime) = &runtime {
        runtime.request_tray_preview();
    }
    let queued = on_main(app, move |app| {
        if !show_popup_inner(app, hold) && !hold {
            if let Some(runtime) = app.try_state::<Runtime>() {
                runtime.release_tray_preview();
            }
            show_main_inner(app, "summary");
        }
    })
    .is_ok();
    if !queued
        && !hold
        && let Some(runtime) = runtime
    {
        runtime.release_tray_preview();
    }
    queued
}

fn show_popup_inner(app: &AppHandle, hold: bool) -> bool {
    request_overlay_mode(app, POPUP);
    let intent = Intent::Show {
        label: POPUP,
        event: Some(("popup-open", Some(hold))),
        focus: !hold,
    };
    if defer_destroying(app, POPUP, &intent) {
        return true;
    }
    if let Some(state) = app.try_state::<crate::commands::AppState>() {
        state
            .popup_hold
            .store(hold, std::sync::atomic::Ordering::SeqCst);
    }
    let Some(window) = layout_popup(app) else {
        return false;
    };
    let _ = window;
    request_show(app, POPUP, intent);
    true
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

pub fn show_tooltip(app: &AppHandle, hold: bool) {
    let _ = on_main(app, move |app| show_tooltip_inner(app, hold));
}

#[derive(Clone, Copy)]
enum TooltipAction {
    Show { hold: bool },
    HidePreview,
    HideHover,
}

fn update_tooltip_state(
    tooltip_hold: &AtomicBool,
    tooltip_closing: &AtomicBool,
    action: TooltipAction,
) -> bool {
    match action {
        TooltipAction::Show { hold } => {
            tooltip_hold.store(hold, Ordering::SeqCst);
            tooltip_closing.store(false, Ordering::SeqCst);
        }
        TooltipAction::HidePreview => tooltip_hold.store(false, Ordering::SeqCst),
        TooltipAction::HideHover => return !tooltip_hold.load(Ordering::SeqCst),
    }
    true
}

fn show_tooltip_inner(app: &AppHandle, hold: bool) {
    if !tooltip_can_show(app, hold, cursor_over_tray(app))
        || (!hold
            && (is_window_visible(app, POPUP)
                || is_window_visible(app, ADD)
                || is_window_visible(app, MENU)
                || app
                    .try_state::<crate::commands::AppState>()
                    .is_some_and(|state| state.tooltip_hold.load(Ordering::SeqCst))))
    {
        return;
    }
    request_overlay_mode(app, TOOLTIP);
    if let Some(state) = app.try_state::<crate::commands::AppState>() {
        update_tooltip_state(
            &state.tooltip_hold,
            &state.tooltip_closing,
            TooltipAction::Show { hold },
        );
    }
    let intent = Intent::Show {
        label: TOOLTIP,
        event: Some(("tooltip-open", Some(hold))),
        focus: false,
    };
    if defer_destroying(app, TOOLTIP, &intent) {
        return;
    }
    let Some(window) = layout_tooltip(app) else {
        return;
    };
    let _ = window;
    request_show(app, TOOLTIP, intent);
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
    let _ = on_main(app, hide_tooltip_inner);
}

/// Releases a settings preview hold in the same queue as tooltip show requests.
pub fn hide_tooltip_preview(app: &AppHandle) {
    let _ = on_main(app, |app| {
        if let Some(state) = app.try_state::<crate::commands::AppState>() {
            update_tooltip_state(
                &state.tooltip_hold,
                &state.tooltip_closing,
                TooltipAction::HidePreview,
            );
        }
        hide_tooltip_inner(app);
    });
}

fn hide_tooltip_inner(app: &AppHandle) {
    if let Some(state) = app.try_state::<crate::commands::AppState>()
        && !update_tooltip_state(
            &state.tooltip_hold,
            &state.tooltip_closing,
            TooltipAction::HideHover,
        )
    {
        return;
    }
    if let Some(runtime) = app.try_state::<Runtime>() {
        if let Ok(mut pending) = runtime.pending.lock() {
            cancel_intent(&mut pending, TOOLTIP);
        }
        if let Ok(mut deferred) = runtime.deferred.lock() {
            cancel_intent(&mut deferred, TOOLTIP);
        }
        if runtime.overlay_mode() != OverlayMode::Tooltip {
            return;
        }
        if runtime.active_overlay_mode() != Some(OverlayMode::Tooltip) {
            return;
        }
    }
    let Some(state) = app.try_state::<crate::commands::AppState>() else {
        return;
    };
    if !is_window_visible(app, TOOLTIP) {
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
    let _ = on_main(app, show_tray_menu_inner);
}

fn show_tray_menu_inner(app: &AppHandle) {
    hide_inner(app, TOOLTIP);
    hide_popup_inner(app);
    if defer_destroying(
        app,
        MENU,
        &Intent::Show {
            label: MENU,
            event: None,
            focus: true,
        },
    ) {
        return;
    }
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
    let _ = window;
    request_show(
        app,
        MENU,
        Intent::Show {
            label: MENU,
            event: Some(("tray-menu-open", None)),
            focus: true,
        },
    );
}

/// 打开卡片设置窗：首次贴主窗右侧（空间不足则左侧），之后保留用户拖动的位置。
pub fn show_inspector(app: &AppHandle) -> Result<(), String> {
    on_main(app, |app| {
        if let Err(err) = show_inspector_inner(app) {
            eprintln!("[show_inspector] {err}");
        }
    })
}

fn show_inspector_inner(app: &AppHandle) -> Result<(), String> {
    if defer_destroying(
        app,
        INSPECTOR,
        &Intent::Show {
            label: INSPECTOR,
            event: None,
            focus: true,
        },
    ) {
        return Ok(());
    }
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
    let restored = restore_position(app, &window);
    if first && !restored {
        let factor = window.scale_factor().unwrap_or(1.0);
        let width = (INSPECTOR_W * scale * factor).round() as i32;
        let height = (INSPECTOR_H * scale * factor).round() as i32;
        place_beside_main(app, &window, width, height);
    }
    request_show(
        app,
        INSPECTOR,
        Intent::Show {
            label: INSPECTOR,
            event: Some(("inspector-open", None)),
            focus: true,
        },
    );
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
    let _ = on_main(app, |app| {
        hide_inner(app, INSPECTOR);
        if let Some(window) = app.get_webview_window(INSPECTOR) {
            let _ = window.emit("inspector-close", ());
        }
    });
}

pub fn show_bg_editor(app: &AppHandle) -> Result<(), String> {
    on_main(app, |app| {
        if let Err(err) = show_bg_editor_inner(app) {
            eprintln!("[show_bg_editor] {err}");
        }
    })
}

fn show_bg_editor_inner(app: &AppHandle) -> Result<(), String> {
    if defer_destroying(
        app,
        BG,
        &Intent::Show {
            label: BG,
            event: None,
            focus: true,
        },
    ) {
        return Ok(());
    }
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
    let restored = restore_position(app, &window);
    if first && !restored {
        let factor = window.scale_factor().unwrap_or(1.0);
        let width = (BG_W * scale * factor).round() as i32;
        let height = (BG_H * scale * factor).round() as i32;
        place_beside_main(app, &window, width, height);
    }
    request_show(
        app,
        BG,
        Intent::Show {
            label: BG,
            event: Some(("bg-editor-open", None)),
            focus: true,
        },
    );
    Ok(())
}

pub fn hide_bg_editor(app: &AppHandle) {
    let _ = on_main(app, |app| {
        hide_inner(app, BG);
        if let Some(window) = app.get_webview_window(BG) {
            let _ = window.emit("bg-editor-close", ());
        }
    });
}

pub fn show_add(app: &AppHandle) -> Result<(), String> {
    on_main(app, |app| {
        if let Err(err) = show_add_inner(app) {
            eprintln!("[show_add] {err}");
        }
    })
}

fn show_add_inner(app: &AppHandle) -> Result<(), String> {
    if defer_destroying(
        app,
        ADD,
        &Intent::Show {
            label: ADD,
            event: None,
            focus: true,
        },
    ) {
        return Ok(());
    }
    let window = ensure_window(app, ADD, 459.0, 586.5, false, false).map_err(|err| {
        eprintln!("[show_add] ensure failed: {err}");
        err.to_string()
    })?;
    if !restore_position(app, &window) {
        let _ = window.center();
    }
    request_show(
        app,
        ADD,
        Intent::Show {
            label: ADD,
            event: Some(("add-open", None)),
            focus: true,
        },
    );
    Ok(())
}

pub fn show_alert_event(app: &AppHandle, event: &str, payload: serde_json::Value) {
    if event == "alert-show"
        && let Some(runtime) = app.try_state::<Runtime>()
        && let Ok(mut visible) = runtime.alert_visible.lock()
    {
        *visible = false;
    }
    let event = event.to_owned();
    let _ = on_main(app, move |app| show_alert_event_inner(app, event, payload));
}

fn show_alert_event_inner(app: &AppHandle, event: String, payload: serde_json::Value) {
    let payload = if event == "alert-show" && payload.is_null() {
        app.try_state::<crate::commands::AppState>()
            .and_then(|state| {
                state.current_alert.lock().ok().and_then(|alert| {
                    alert
                        .as_ref()
                        .and_then(|alert| serde_json::to_value(alert).ok())
                })
            })
            .unwrap_or(serde_json::Value::Null)
    } else {
        payload
    };
    let intent = Intent::Alert { event, payload };
    if defer_destroying(app, ALERT, &intent) {
        return;
    }
    if app
        .try_state::<Runtime>()
        .and_then(|runtime| runtime.ready.lock().ok().map(|ready| ready.contains(ALERT)))
        .unwrap_or(false)
    {
        apply_intent(app, intent);
        return;
    }
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
    set_alert_position_mode_window(&window, false);
    let (sw, sh) = winutil::primary_screen_size();
    let work = winutil::monitor_and_work_area_for_point(sw / 2, sh / 2)
        .map(|(_, work)| work)
        .unwrap_or(winutil::Rect {
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
    request_show(app, ALERT, intent);
}

pub fn wait_alert_visible(app: &AppHandle, timeout: Duration) -> bool {
    let Some(runtime) = app.try_state::<Runtime>() else {
        return false;
    };
    let Ok(visible) = runtime.alert_visible.lock() else {
        return false;
    };
    let result = runtime
        .alert_shown
        .wait_timeout_while(visible, timeout, |visible| !*visible);
    result.map(|(visible, _)| *visible).unwrap_or(false)
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
    let _ = on_main(app, move |app| set_alert_position_mode_inner(app, enabled));
}

fn set_alert_position_mode_inner(app: &AppHandle, enabled: bool) {
    let Some(window) = app.get_webview_window(ALERT) else {
        return;
    };
    set_alert_position_mode_window(&window, enabled);
}

fn set_alert_position_mode_window(window: &WebviewWindow, enabled: bool) {
    let _ = window.set_ignore_cursor_events(!enabled);
    let _ = set_utility_styles(window, !enabled, !enabled);
    if enabled {
        let _ = window.set_focus();
    }
}

pub fn hide(app: &AppHandle, label: &str) {
    let label = label.to_owned();
    let _ = on_main(app, move |app| hide_inner(app, &label));
}

fn hide_inner(app: &AppHandle, label: &str) {
    let physical = physical_label(label);
    let should_hide = app
        .try_state::<Runtime>()
        .map(|runtime| runtime.prepare_hide(label))
        .unwrap_or_else(|| OverlayMode::from_label(label).is_none());
    if !should_hide {
        return;
    }
    if let Some(window) = app.get_webview_window(physical) {
        let _ = window.hide();
        set_inactive(&window, true);
        if let Ok(position) = window.outer_position()
            && let Some(runtime) = app.try_state::<Runtime>()
            && let Ok(mut positions) = runtime.positions.lock()
        {
            positions.insert(physical.to_owned(), position);
        }
    }
    if let Some(runtime) = app.try_state::<Runtime>() {
        if let Ok(mut hidden) = runtime.hidden_since.lock() {
            hidden.remove(physical);
        }
        if let Some(mode) = OverlayMode::from_label(label)
            && let Ok(mut active) = runtime.active_overlay_mode.lock()
            && Some(mode) == *active
        {
            *active = None;
        }
        if label == ALERT
            && let Ok(mut visible) = runtime.alert_visible.lock()
        {
            *visible = false;
        }
    }
}

/// Rust 侧直接收起预览小窗时通知前端复位（否则下一轮 popup-open 会误判仍在打开）。
pub fn hide_popup(app: &AppHandle) {
    let _ = on_main(app, hide_popup_inner);
}

fn hide_popup_inner(app: &AppHandle) {
    hide_inner(app, POPUP);
    if let Some(window) = app.get_webview_window(POPUP) {
        let _ = window.emit("popup-close", ());
    }
}

pub fn tray_overlay_mode(app: &AppHandle) -> &'static str {
    app.try_state::<Runtime>()
        .map(|runtime| runtime.overlay_mode().label())
        .unwrap_or(POPUP)
}

pub fn is_window_visible(app: &AppHandle, label: &str) -> bool {
    let visible = app
        .get_webview_window(physical_label(label))
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    let mode = app
        .try_state::<Runtime>()
        .and_then(|runtime| runtime.active_overlay_mode())
        .unwrap_or_default();
    logical_window_visible(mode, label, visible)
}

/// 缩放、设备刷新或显示设置变化后，按当前可见行数重排已显示的预览小窗。
/// 重排不是「打开」：不发 `popup-open`，否则前端会再请求一次整页刷新。
pub fn relayout_popup(app: &AppHandle) {
    let _ = on_main(app, |app| {
        if is_window_visible(app, POPUP) {
            let _ = layout_popup(app);
        }
    });
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
    let _ = on_main(app, |app| {
        match tooltip_relayout_action(is_window_visible(app, TOOLTIP)) {
            TooltipRelayoutAction::Skip => {}
            TooltipRelayoutAction::Layout => {
                let _ = layout_tooltip(app);
            }
        }
    });
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
    let physical = physical_label(label);
    if let Some(runtime) = app.try_state::<Runtime>()
        && runtime
            .destroying
            .lock()
            .ok()
            .map(|set| set.contains(physical))
            .unwrap_or(false)
    {
        return Err(tauri::Error::InvalidWebviewUrl(
            "window destruction in progress",
        ));
    }
    if let Some(window) = app.get_webview_window(physical) {
        if let Some(mode) = OverlayMode::from_label(label) {
            let active = app
                .try_state::<Runtime>()
                .and_then(|runtime| runtime.active_overlay_mode());
            if active != Some(mode) {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                    set_inactive(&window, true);
                }
                let _ = window.set_focusable(!no_activate);
                let _ = window.set_ignore_cursor_events(passive);
            }
        }
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
    let browser_args =
        std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
    let mut builder =
        WebviewWindowBuilder::new(app, physical, WebviewUrl::App("index.html".into()))
            .additional_browser_args(&crate::browser_arguments(&browser_args))
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
    let overlay = OverlayMode::from_label(label).is_some();
    if overlay {
        builder = builder.focusable(!no_activate);
    }
    let window = builder.build()?;
    register_created(app, &window);
    let _ = restore_position(app, &window);
    if !overlay && let Err(err) = set_utility_styles(&window, passive, no_activate) {
        eprintln!("[window] {label} style: {err}");
    }
    if passive {
        let _ = window.set_ignore_cursor_events(true);
    }
    Ok(window)
}

#[cfg(windows)]
fn utility_extended_style(current: isize, passive: bool, no_activate: bool) -> Option<isize> {
    use windows::Win32::UI::WindowsAndMessaging::{
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };
    let mut updated = current
        & !(WS_EX_NOACTIVATE.0 as isize
            | WS_EX_TOOLWINDOW.0 as isize
            | WS_EX_TRANSPARENT.0 as isize);
    if no_activate {
        updated |= WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize;
    }
    if passive {
        updated |= WS_EX_TRANSPARENT.0 as isize;
    }
    (updated != current).then_some(updated)
}

fn set_utility_styles(
    window: &WebviewWindow,
    passive: bool,
    no_activate: bool,
) -> tauri::Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, SET_WINDOW_POS_FLAGS, SWP_FRAMECHANGED, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
        };
        let hwnd = window.hwnd()?;
        let current = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        if let Some(updated) = utility_extended_style(current, passive, no_activate) {
            unsafe {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, updated);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SET_WINDOW_POS_FLAGS(
                        SWP_FRAMECHANGED.0
                            | SWP_NOACTIVATE.0
                            | SWP_NOMOVE.0
                            | SWP_NOSIZE.0
                            | SWP_NOZORDER.0,
                    ),
                );
            }
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
    let (monitor, work) = winutil::monitor_and_work_area_for_point(
        (rect.left + rect.right) / 2,
        (rect.top + rect.bottom) / 2,
    )?;
    Some(winutil::anchor_for_rect(
        rect, width, height, gap, monitor, work,
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
