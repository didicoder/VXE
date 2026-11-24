//! 配置层（V16 `src/store.py` 的移植）。
//!
//! `config.json` 固定在程序同级目录，整个文件夹拷走就带着配置和全部运行时文件；
//! 不再回退 `%APPDATA%`。**不自动继承旧版 `%APPDATA%` 配置**：
//! 旧配置里的历史缩放/设备会在首次启动时把新默认覆盖掉。
//! 这里只做路径解析 / 读写 / 设备归一化 / 夹取，不碰注册表也不碰 UI。
//!
//! 设备字段：`id` 唯一键（= 旧版 `key`）、`label` 显示名、`vid`/`pid` 接收器、
//! `wired_pid` 插线后 ID、`profile` 协议 id、`color` 强调色、`colors` 三档电量配色
//! （只记用户改过的档位）、`link` 连接方式、`bt_addr` 蓝牙音频 MAC。

use std::collections::HashSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock};

use serde_json::{Map, Value, json};

pub const CONFIG_NAME: &str = "config.json";

/// 强调色顺序：新设备按 index 取色，与 `theme.PALETTE` 一致。
pub const PALETTE: [&str; 6] = ["blue", "red", "green", "purple", "amber", "cyan"];
pub const BANDS: [&str; 3] = ["good", "mid", "low"];
pub const DEVICE_ICONS: [&str; 6] = [
    "mouse", "keyboard", "headset", "earbuds", "gamepad", "phone",
];

/// 出厂配色：设备里与它相同的档位等于「没配过」，必须丢掉，否则全局色改不动。
const BAND_FACTORY: [(&str, &str); 3] = [("good", "green"), ("mid", "blue"), ("low", "red")];

/// 默认配置：键名与 V16 `store.DEFAULTS` 对齐；设备列表出厂为空，由用户自行添加。
pub static DEFAULTS: LazyLock<Map<String, Value>> = LazyLock::new(|| {
    let defaults = json!({
        "autostart": false,
        "refresh_seconds": 60,
        "hover_delay": 0.3,
        "good_level": 80,
        "warn_level": 20,
        "band_colors": {"good": "green", "mid": "blue", "low": "red"},
        "ui_base": 1.0,
        "ui_scale": 1.0,
        "add_base": 1.0,
        "popup_base": 1.0,
        "popup_scale": 1.0,
        "tip_base": 1.0,
        "tip_scale": 1.0,
        "split_scales": true,
        "indicator_style": "ring",
        "ui_style": "photo",
        "photo_cols": 3,
        "sidebar_collapsed": false,
        "display_summary": "connected",
        "display_devices": "connected",
        "display_popup": "all",
        "display_tip": "all",
        "ui_theme": "light",
        "theme_mode": "manual",
        "theme_light_at": "07:00",
        "theme_dark_at": "19:00",
        "notify_low": true,
        "notify_level": 20,
        "notify_interval_minutes": 0,
        "alert_mode": "tray",
        "tray_blink_seconds": 10,
        "tray_level_mode": "average",
        "tray_level_device": "",
        "tray_icon_style": "bolt",
        "alert_base": 1.0,
        "alert_scale": 1.0,
        "alert_sound": true,
        "alert_sound_file": "notify_plat.wav",
        "alert_title": "",
        "alert_subtitle": "",
        "alert_pos": null,
        "bg_ratio": 1.0,
        "bg_dx": 0.0,
        "bg_dy": 0.0,
        "bg_blur": 12.0,
        "bg_dim": 0.0,
        "hotkey_enabled": false,
        "hotkey": "ctrl+alt+d",
        "last_levels": {},
        // 出厂设备列表为空：设备由用户自己添加，不预置任何硬件。
        "devices": [],
    });
    match defaults {
        Value::Object(map) => map,
        other => {
            debug_assert!(false, "DEFAULTS 必须是对象，实际是 {other}");
            Map::new()
        }
    }
});

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("读取配置失败 {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("写入配置失败 {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("配置不是合法 JSON {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("配置顶层不是对象 {path}")]
    NotObject { path: PathBuf },
    #[error("序列化配置失败: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("设备项不是对象")]
    DeviceNotObject,
    #[error("该设备已在列表中")]
    DuplicateDevice,
}

/// 界面明暗（`ui_theme`）。V16 用大小写敏感比较，只有 `"light"` 算亮色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiTheme {
    #[default]
    Dark,
    Light,
}

impl UiTheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

impl std::fmt::Display for UiTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 主题来源（`theme_mode`）。大小写不敏感，非法值当 `manual`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Manual,
    System,
    Schedule,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::System => "system",
            Self::Schedule => "schedule",
        }
    }
}

impl std::fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 当前进程的可执行文件（V16 `exe_path()`）。
pub fn exe_path() -> PathBuf {
    match env::current_exe() {
        Ok(path) => absolutize(path),
        Err(_) => absolutize(PathBuf::from(".")),
    }
}

/// 程序目录：exe 所在目录（资源、`sound/`、便携配置都在这里找）。
pub fn program_dir() -> PathBuf {
    match exe_path().parent() {
        Some(dir) => dir.to_path_buf(),
        None => absolutize(PathBuf::from(".")),
    }
}

/// 配置目录：固定在程序同级；`VXE_CONFIG_DIR` 等环境变量仅供测试显式覆盖。
/// 结果整个进程只解析一次。
pub fn app_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| env_override().unwrap_or_else(program_dir))
}

pub fn config_path() -> PathBuf {
    app_dir().join(CONFIG_NAME)
}

fn env_override() -> Option<PathBuf> {
    for key in ["VXE_CONFIG_DIR", "EZB_CONFIG_DIR", "BP_CONFIG_DIR"] {
        if let Some(value) = env::var_os(key).filter(|value| !value.is_empty()) {
            return Some(absolutize(PathBuf::from(value)));
        }
    }
    None
}

fn absolutize(path: PathBuf) -> PathBuf {
    std::path::absolute(&path).unwrap_or(path)
}

#[derive(Debug)]
struct LegacyScales {
    ui_scale: f64,
    popup_scale: f64,
    tip_scale: f64,
}

/// 配置实例（V16 的 `Store`）。数据全在实例里，没有全局可变状态。
#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    data: Map<String, Value>,
    load_error: Option<StoreError>,
    /// 旧配置（没有 `ui_base`）里的三个倍率，首次升级用。
    legacy_scales: Option<LegacyScales>,
    /// 旧版三处窗口共用 `ui_scale`，要拆成各自的基准。
    need_split: bool,
}

impl Store {
    /// 读正式配置。
    pub fn load() -> Self {
        Self::load_at(config_path())
    }

    /// 读指定路径的配置，不做旧配置搬迁（测试 / 自定义路径用）。
    ///
    /// 读失败不返回错误：数据退回默认值，错误留在 [`Store::load_error`] 里。
    pub fn load_at(path: impl Into<PathBuf>) -> Self {
        let mut store = Self {
            path: path.into(),
            data: DEFAULTS.clone(),
            load_error: None,
            legacy_scales: None,
            need_split: false,
        };
        if store.read() {
            // 升级 / 设备归一化的回写：失败只记录，不拦住启动（V16 同样吞掉）。
            if let Err(error) = store.save() {
                store.load_error.get_or_insert(error);
            }
        }
        store
    }

    /// 整份配置（给「读取全部设置」这类 IPC 用）。
    pub fn data(&self) -> &Map<String, Value> {
        &self.data
    }

    /// 读原始值；键不存在时退回 [`DEFAULTS`]（V16 `Store.get` 的语义）。
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key).or_else(|| DEFAULTS.get(key))
    }

    /// 写一个键并立刻落盘（V16 每次 `set` 都写文件）。
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), StoreError> {
        self.data.insert(key.to_owned(), value);
        self.save()
    }

    /// 先写 `config.json.tmp` 再 rename，崩溃不会留下半截配置。
    ///
    /// serde_json 不转义非 ASCII（没有开关），等价于 V16 的 `ensure_ascii=False`。
    pub fn save(&self) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| StoreError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let text = serde_json::to_string_pretty(&self.data).map_err(StoreError::Serialize)?;
        let tmp = with_tmp_suffix(&self.path);
        fs::write(&tmp, text.as_bytes()).map_err(|source| StoreError::Write {
            path: tmp.clone(),
            source,
        })?;
        fs::rename(&tmp, &self.path).map_err(|source| StoreError::Write {
            path: self.path.clone(),
            source,
        })
    }

    /// 老配置首次升级：把「现在看到的大小」定为 1×。
    ///
    /// 三个 base 记下当时的倍率、scale 归 1.0，升级后像素尺寸完全不变，
    /// 设置页显示成 1.00×。由 `load` 发现配置里没有 `ui_base` 后触发，只跑一次。
    pub fn migrate_scales(&mut self) -> Result<bool, StoreError> {
        let Some((ui, popup, tip)) = self
            .legacy_scales
            .as_ref()
            .map(|old| (old.ui_scale, old.popup_scale, old.tip_scale))
        else {
            return Ok(false);
        };
        self.data
            .insert("ui_base".to_owned(), Value::from(clamp_f(ui, 0.4, 2.0)));
        self.data.insert(
            "popup_base".to_owned(),
            Value::from(clamp_f(popup, 0.5, 2.0)),
        );
        self.data
            .insert("tip_base".to_owned(), Value::from(clamp_f(tip, 0.5, 2.0)));
        self.data.insert("ui_scale".to_owned(), Value::from(1.0));
        self.data.insert("popup_scale".to_owned(), Value::from(1.0));
        self.data.insert("tip_scale".to_owned(), Value::from(1.0));
        self.save()?;
        self.legacy_scales = None;
        Ok(true)
    }

    /// 把预览小窗 / 悬停卡从 `ui_scale` 里拆出来：把当时的 ui 基准并进各自的 base，
    /// 物理尺寸不变，之后「界面缩放」只影响主面板。只在第一次跑。
    pub fn split_surfaces(&mut self) -> Result<bool, StoreError> {
        if !self.need_split {
            return Ok(false);
        }
        let ui = self.ui_base();
        let popup = round3(clamp_f(ui * self.popup_base(), 0.5, 2.0));
        let tip = round3(clamp_f(ui * self.tip_base(), 0.5, 2.0));
        self.data
            .insert("popup_base".to_owned(), Value::from(popup));
        self.data.insert("tip_base".to_owned(), Value::from(tip));
        self.data
            .insert("split_scales".to_owned(), Value::Bool(true));
        self.save()?;
        self.need_split = false;
        Ok(true)
    }

    /// E3：删除 V17 不再使用的配置键，先备份原文件并写迁移日志。
    ///
    /// `pin_tray/swap_on` 是 V16 死键；`render_direct/render_sharpen` 是 V16 渲染
    /// 实验键，CSS 方案没有对应物。未知键仍然保留（前进兼容）。
    pub fn cleanup_dead_keys(&mut self) -> Result<Vec<String>, StoreError> {
        const DEAD: [&str; 4] = ["pin_tray", "swap_on", "render_direct", "render_sharpen"];
        let removed: Vec<String> = DEAD
            .iter()
            .filter(|key| self.data.contains_key(**key))
            .map(|key| (*key).to_string())
            .collect();
        if removed.is_empty() {
            return Ok(removed);
        }
        if self.path.exists() {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or(0);
            let backup = self.path.with_file_name(format!("config.json.bak-{stamp}"));
            fs::copy(&self.path, &backup).map_err(|source| StoreError::Write {
                path: backup,
                source,
            })?;
        }
        let mut lines = Vec::new();
        for key in &removed {
            let value = self
                .data
                .remove(key)
                .map(|value| serde_json::to_string(&value).unwrap_or_default())
                .unwrap_or_default();
            lines.push(format!("removed {key}={value}"));
        }
        self.save()?;
        if let Some(parent) = self.path.parent() {
            let log = parent.join("config-migration.log");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
            {
                use std::io::Write as _;
                let _ = writeln!(file, "{}", lines.join("\n"));
            }
        }
        Ok(removed)
    }

    /// 2026-11-24：把旧内置显示名一次性迁到中文名。
    ///
    /// 只替换与旧内置名完全相同的 label；用户自定义名、照片卡文案、
    /// 墓碑键和 last_levels 全部不动。空 label / 等于 id 的占位补成唯一可识别的名称。
    pub fn migrate_device_labels(&mut self) -> Result<bool, StoreError> {
        const VERSION_KEY: &str = "label_migration_v1";
        const VERSION: &str = "2026-11-24";
        if self.data.get(VERSION_KEY).and_then(Value::as_str) == Some(VERSION) {
            return Ok(false);
        }
        let mut changes: Vec<(String, String, String)> = Vec::new();
        let catalog = crate::battery::models::load(&std::collections::HashMap::new());
        if let Some(Value::Array(items)) = self.data.get_mut("devices") {
            for dev in items.iter_mut() {
                let Some(obj) = dev.as_object_mut() else {
                    continue;
                };
                if obj
                    .get("bt_addr")
                    .and_then(Value::as_str)
                    .is_some_and(|addr| !addr.trim().is_empty())
                {
                    continue;
                }
                let profile = obj
                    .get("profile")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if profile.eq_ignore_ascii_case("xinput") {
                    continue;
                }
                let model = obj
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let id = obj
                    .get("id")
                    .or_else(|| obj.get("key"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let old = obj
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let target = if old.is_empty() || old == id {
                    placeholder_label(&catalog, &model, &profile)
                } else {
                    legacy_label(&catalog, &model, &profile, &old)
                };
                let Some(new) = target else {
                    continue;
                };
                if new == old {
                    continue;
                }
                obj.insert("label".to_owned(), Value::String(new.clone()));
                changes.push((if id.is_empty() { "?".to_owned() } else { id }, old, new));
            }
        }
        self.data
            .insert(VERSION_KEY.to_owned(), Value::String(VERSION.to_owned()));
        if changes.is_empty() {
            self.save()?;
            return Ok(false);
        }
        if self.path.exists() {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or(0);
            let backup = self.path.with_file_name(format!("config.json.bak-{stamp}"));
            fs::copy(&self.path, &backup).map_err(|source| StoreError::Write {
                path: backup,
                source,
            })?;
        }
        self.save()?;
        if let Some(parent) = self.path.parent() {
            let log = parent.join("config-migration.log");
            if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(log) {
                use std::io::Write as _;
                for (id, old, new) in &changes {
                    let _ = writeln!(file, "label: {id} \"{old}\" -> \"{new}\"");
                }
            }
        }
        Ok(true)
    }

    // ---------- 夹取 getter（UI 只从这里取值） ----------

    /// 旧安装基准（仅旧配置迁移/兼容读取，不参与新窗口尺寸）。
    pub fn ui_base(&self) -> f64 {
        clamp_f(self.num_or("ui_base", 1.0), 0.4, 2.0)
    }

    /// 旧预览窗基准（仅旧配置迁移/兼容读取，不参与新窗口尺寸）。
    pub fn popup_base(&self) -> f64 {
        clamp_f(self.num_or("popup_base", 1.0), 0.5, 2.0)
    }

    /// 旧悬停基准（仅旧配置迁移/兼容读取，不参与新窗口尺寸）。
    pub fn tip_base(&self) -> f64 {
        clamp_f(self.num_or("tip_base", 1.0), 0.5, 2.0)
    }

    pub fn popup_scale(&self) -> f64 {
        clamp_f(scale_num(self.get("popup_scale")), 0.5, 2.0)
    }

    pub fn tip_scale(&self) -> f64 {
        clamp_f(scale_num(self.get("tip_scale")), 0.5, 2.0)
    }

    /// 主窗口缩放。V16 在 `surface.set_ui_scale` 里处理：越界不是夹取，而是退回 1.0。
    pub fn ui_scale(&self) -> f64 {
        let value = scale_num(self.get("ui_scale"));
        if (0.5..=2.0).contains(&value) {
            value
        } else {
            1.0
        }
    }

    pub fn alert_scale(&self) -> f64 {
        clamp_f(scale_num(self.get("alert_scale")), 0.5, 2.0)
    }

    /// 低电量通知阈值：夹在 [5, 50]（比「电量不足」下限更保守）。
    pub fn notify_level(&self) -> i32 {
        to_i32(clamp_i(self.int_or("notify_level", 20), 5, 50))
    }

    /// 低电量重复提醒间隔（分钟）：0 = 仅提醒一次，夹在 [0, 120]。
    pub fn notify_interval_minutes(&self) -> i32 {
        to_i32(clamp_i(self.int_or("notify_interval_minutes", 0), 0, 120))
    }

    /// 背景图缩放（cover 基准之上）：夹在 [1.0, 3.0]。
    pub fn bg_ratio(&self) -> f64 {
        clamp_f(self.num_or("bg_ratio", 1.0), 1.0, 3.0)
    }

    /// 背景画面水平/垂直平移（百分比）：夹在 [-50, 50]，与缩放相互独立。
    pub fn bg_dx(&self) -> f64 {
        clamp_f(self.num_or("bg_dx", 0.0), -50.0, 50.0)
    }

    pub fn bg_dy(&self) -> f64 {
        clamp_f(self.num_or("bg_dy", 0.0), -50.0, 50.0)
    }

    /// 自定义壁纸的磨砂强度（px）：夹在 [0, 24]。
    pub fn bg_blur(&self) -> f64 {
        clamp_f(self.num_or("bg_blur", 12.0), 0.0, 24.0)
    }

    /// 自定义壁纸的暗化强度（0–60 百分比）。
    pub fn bg_dim(&self) -> f64 {
        clamp_f(self.num_or("bg_dim", 0.0), 0.0, 60.0)
    }

    /// 托盘红色闪烁时长（秒），与设置页步进范围一致。
    pub fn tray_blink_seconds(&self) -> u64 {
        clamp_i(self.int_or("tray_blink_seconds", 10), 5, 120) as u64
    }

    /// 托盘中闪电图标取哪一路电量：`average` / `device` / `min`。
    pub fn tray_level_mode(&self) -> &'static str {
        match self.get("tray_level_mode").and_then(Value::as_str) {
            Some("device") => "device",
            Some("min") => "min",
            _ => "average",
        }
    }

    /// 自选设备的 id；空串 = 尚未选择。
    pub fn tray_level_device(&self) -> String {
        self.get("tray_level_device")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    /// 托盘图标样式：`bolt` / `ring` / `battery`，脏值回落闪电。
    pub fn tray_icon_style(&self) -> &'static str {
        match self.get("tray_icon_style").and_then(Value::as_str) {
            Some("ring") => "ring",
            Some("battery") => "battery",
            _ => "bolt",
        }
    }

    /// 刷新间隔（秒）。V16 只在 UI 步进处夹取，这里作为唯一入口统一夹取。
    pub fn refresh_seconds(&self) -> u64 {
        clamp_i(self.int_or("refresh_seconds", 60), 15, 600) as u64
    }

    /// 悬停展开延时（秒）。
    pub fn hover_delay(&self) -> f64 {
        clamp_f(self.num_or("hover_delay", 0.3), 0.0, 2.0)
    }

    /// 「电量不足」下限；上界 95 是为了让 good ≥ warn+5 还能落在 100 以内。
    pub fn warn_level(&self) -> i32 {
        to_i32(clamp_i(self.int_or("warn_level", 20), 0, 95))
    }

    /// 「电量充足」下限：始终至少比 warn 高 5。
    pub fn good_level(&self) -> i32 {
        let good = clamp_i(self.int_or("good_level", 80), 0, 100);
        to_i32(good.max(i64::from(self.warn_level()) + 5))
    }

    pub fn alert_sound(&self) -> bool {
        truthy(self.get("alert_sound"))
    }

    /// 低电量提醒方式（`alert_mode`）：`popup` / `tray` / `both`，脏值回 `popup`。
    pub fn alert_mode(&self) -> &'static str {
        match self.get("alert_mode").and_then(Value::as_str) {
            Some("tray") => "tray",
            Some("both") => "both",
            _ => "popup",
        }
    }

    /// 提示音文件名：只留 basename 且必须以 `.wav` 结尾；空 = 用找到的第一个。
    pub fn alert_sound_file(&self) -> String {
        let Some(Value::String(raw)) = self.get("alert_sound_file") else {
            return String::new();
        };
        let name = basename(raw.trim());
        if name.to_lowercase().ends_with(".wav") {
            name.to_owned()
        } else {
            String::new()
        }
    }

    /// 自定义弹窗中心（物理像素）；不是两个整数就当作没设。
    pub fn alert_pos(&self) -> Option<(i32, i32)> {
        let Some(Value::Array(items)) = self.get("alert_pos") else {
            return None;
        };
        if items.len() != 2 {
            return None;
        }
        Some((to_i32(try_int(&items[0])?), to_i32(try_int(&items[1])?)))
    }

    pub fn ui_theme(&self) -> UiTheme {
        match self.get("ui_theme") {
            Some(Value::String(value)) if value == "light" => UiTheme::Light,
            _ => UiTheme::Dark,
        }
    }

    pub fn theme_mode(&self) -> ThemeMode {
        let Some(Value::String(value)) = self.get("theme_mode") else {
            return ThemeMode::Manual;
        };
        if value.eq_ignore_ascii_case("system") {
            ThemeMode::System
        } else if value.eq_ignore_ascii_case("schedule") {
            ThemeMode::Schedule
        } else {
            ThemeMode::Manual
        }
    }

    // ---------- 设备 ----------

    /// 归一化后的设备列表（`load` 已保证是数组）。
    pub fn devices(&self) -> &[Value] {
        self.data
            .get("devices")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// 整体替换设备列表并落盘（每台都重新归一化，id 冲突自动改名）。
    pub fn save_devices(&mut self, devices: Vec<Value>) -> Result<(), StoreError> {
        let mut used = HashSet::new();
        let normalized: Vec<Value> = devices
            .iter()
            .enumerate()
            .filter_map(|(index, dev)| normalize_device(dev, index, &mut used))
            .collect();
        self.data
            .insert("devices".to_owned(), Value::Array(normalized));
        self.save()
    }

    /// 追加一台设备，返回补齐后的设备（V16 `add_device`）。
    pub fn add_device(&mut self, device: Value) -> Result<Value, StoreError> {
        let mut devices = self.devices().to_vec();
        let mut used: HashSet<String> = devices
            .iter()
            .filter_map(|dev| dev.get("id").and_then(Value::as_str).map(str::to_owned))
            .collect();
        let Some(normalized) = normalize_device(&device, devices.len(), &mut used) else {
            return Err(StoreError::DeviceNotObject);
        };
        // 同一硬件只允许一条：HID 按 vid + pid/wired_pid，蓝牙按地址。
        let bt_addr = text_or_empty(normalized.get("bt_addr")).trim().to_owned();
        let vid = text_or_empty(normalized.get("vid")).trim().to_owned();
        let pid = text_or_empty(normalized.get("pid")).trim().to_owned();
        let wired = text_or_empty(normalized.get("wired_pid")).trim().to_owned();
        let model = text_or_empty(normalized.get("model")).trim().to_owned();
        let instance = text_or_empty(normalized.get("instance")).trim().to_owned();
        let duplicate = if !bt_addr.is_empty() {
            self.has_audio(&bt_addr).is_some()
        } else if !vid.is_empty() && (!pid.is_empty() || !wired.is_empty()) {
            devices.iter().any(|dev| {
                let dev_vid = text_or_empty(dev.get("vid"));
                if !dev_vid.eq_ignore_ascii_case(&vid) {
                    return false;
                }
                let dev_pid = text_or_empty(dev.get("pid"));
                let dev_wired = text_or_empty(dev.get("wired_pid"));
                let pid_hit = (!pid.is_empty()
                    && (dev_pid.eq_ignore_ascii_case(&pid)
                        || dev_wired.eq_ignore_ascii_case(&pid)))
                    || (!wired.is_empty()
                        && (dev_pid.eq_ignore_ascii_case(&wired)
                            || dev_wired.eq_ignore_ascii_case(&wired)));
                if !pid_hit {
                    return false;
                }
                let dev_model = text_or_empty(dev.get("model")).trim().to_owned();
                if !model.is_empty()
                    && !dev_model.is_empty()
                    && !dev_model.eq_ignore_ascii_case(&model)
                {
                    return false;
                }
                let dev_instance = text_or_empty(dev.get("instance")).trim().to_owned();
                instance.is_empty()
                    || dev_instance.is_empty()
                    || dev_instance.eq_ignore_ascii_case(&instance)
            })
        } else {
            let id = text_or_empty(normalized.get("id"));
            let key = text_or_empty(normalized.get("key"));
            devices.iter().any(|dev| {
                let dev_id = text_or_empty(dev.get("id"));
                let dev_key = text_or_empty(dev.get("key"));
                (!id.is_empty() && dev_id == id) || (!key.is_empty() && dev_key == key)
            })
        };
        if duplicate {
            return Err(StoreError::DuplicateDevice);
        }
        devices.push(normalized.clone());
        self.save_devices(devices)?;
        Ok(normalized)
    }

    /// 按给定 id 顺序重排设备；不在列表里的设备保留在尾部（不丢）。
    pub fn reorder_devices(&mut self, ids: &[String]) -> Result<(), StoreError> {
        let current = self.devices();
        let mut ordered: Vec<Value> = Vec::with_capacity(current.len());
        for id in ids {
            if let Some(dev) = current
                .iter()
                .find(|dev| dev.get("id").and_then(Value::as_str) == Some(id.as_str()))
                && !ordered.iter().any(|done| done.get("id") == dev.get("id"))
            {
                ordered.push(dev.clone());
            }
        }
        for dev in current {
            if !ordered.iter().any(|done| done.get("id") == dev.get("id")) {
                ordered.push(dev.clone());
            }
        }
        self.save_devices(ordered)
    }

    /// 删设备，同时清掉它的 `last_levels` 记录，并把硬件键记入墓碑
    /// （`removed_devices`）——自动添加会跳过墓碑里的设备，手动添加解除。
    pub fn remove_device(&mut self, id: &str) -> Result<(), StoreError> {
        let removed = self
            .devices()
            .iter()
            .find(|dev| dev.get("id").and_then(Value::as_str) == Some(id))
            .map(hardware_key_of);
        let devices: Vec<Value> = self
            .devices()
            .iter()
            .filter(|dev| dev.get("id").and_then(Value::as_str) != Some(id))
            .cloned()
            .collect();
        self.save_devices(devices)?;
        if let Some(key) = removed {
            self.mark_removed(&key)?;
        }
        let mut levels = match self.get("last_levels") {
            Some(Value::Object(map)) => map.clone(),
            _ => Map::new(),
        };
        levels.remove(id);
        self.data
            .insert("last_levels".to_owned(), Value::Object(levels));
        self.save()
    }

    /// 用户删过的设备硬件键（`hardware_key_of` 格式）。
    pub fn removed_devices(&self) -> Vec<String> {
        self.get("removed_devices")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 记入墓碑（幂等）。
    pub fn mark_removed(&mut self, key: &str) -> Result<(), StoreError> {
        let mut list = self.removed_devices();
        if list.iter().any(|item| item == key) {
            return Ok(());
        }
        list.push(key.to_owned());
        self.data
            .insert("removed_devices".to_owned(), Value::from(list));
        self.save()
    }

    /// 解除墓碑（手动添加时调用）。
    pub fn unmark_removed(&mut self, key: &str) -> Result<(), StoreError> {
        let list = self.removed_devices();
        let filtered: Vec<String> = list.into_iter().filter(|item| item != key).collect();
        if filtered.len() == self.removed_devices().len() {
            return Ok(());
        }
        self.data
            .insert("removed_devices".to_owned(), Value::from(filtered));
        self.save()
    }

    /// 改某台设备的字段（`fields` 即 V16 的 `**kwargs`），然后整体归一化落盘。
    pub fn update_device(
        &mut self,
        id: &str,
        fields: Map<String, Value>,
    ) -> Result<(), StoreError> {
        let mut devices = self.devices().to_vec();
        for dev in &mut devices {
            if dev.get("id").and_then(Value::as_str) == Some(id)
                && let Some(object) = dev.as_object_mut()
            {
                for (key, value) in &fields {
                    object.insert(key.clone(), value.clone());
                }
            }
        }
        self.save_devices(devices)
    }

    /// 按蓝牙地址找已添加的耳机/音箱（它们的 vid/pid 不唯一）。
    pub fn has_audio(&self, addr: &str) -> Option<&Value> {
        let want = addr.trim().to_uppercase();
        if want.is_empty() {
            return None;
        }
        self.devices()
            .iter()
            .find(|dev| text_or_empty(dev.get("bt_addr")).trim().to_uppercase() == want)
    }

    // ---------- 内部 ----------

    fn num_or(&self, key: &str, default: f64) -> f64 {
        num(self.get(key), default)
    }

    fn int_or(&self, key: &str, default: i64) -> i64 {
        int(self.get(key), default)
    }

    /// 合并文件内容并归一化设备；返回是否需要回写。
    fn read(&mut self) -> bool {
        let mut changed = false;
        match fs::read_to_string(&self.path) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(Value::Object(loaded)) => {
                    if !loaded.contains_key("split_scales") {
                        self.need_split = true;
                    }
                    if !loaded.contains_key("ui_base") {
                        self.legacy_scales = Some(LegacyScales {
                            ui_scale: num(loaded.get("ui_scale"), 0.72),
                            popup_scale: num(loaded.get("popup_scale"), 1.0),
                            tip_scale: num(loaded.get("tip_scale"), 1.0),
                        });
                    }
                    // 逐键覆盖：未知键原样保留，前向兼容。
                    for (key, value) in loaded {
                        self.data.insert(key, value);
                    }
                }
                Ok(_) => {
                    self.load_error = Some(StoreError::NotObject {
                        path: self.path.clone(),
                    });
                    // 损坏文件要被默认值覆盖（V16 行为），不能等归一化「有变化」才回写。
                    changed = true;
                }
                Err(source) => {
                    self.load_error = Some(StoreError::Parse {
                        path: self.path.clone(),
                        source,
                    });
                    changed = true;
                }
            },
            // 文件不存在 = 需要落盘：不能只靠「归一化有无变化」判断，
            // 默认值本身已是归一化形态时，否则全新安装写不出 config.json。
            Err(source) if source.kind() == io::ErrorKind::NotFound => changed = true,
            Err(source) => {
                self.load_error = Some(StoreError::Read {
                    path: self.path.clone(),
                    source,
                });
            }
        }

        let mut used = HashSet::new();
        let mut devices = Vec::new();
        if let Some(Value::Array(items)) = self.data.get("devices") {
            for (index, dev) in items.iter().enumerate() {
                if let Some(normalized) = normalize_device(dev, index, &mut used) {
                    devices.push(normalized);
                }
            }
        }
        let normalized = Value::Array(devices);
        if self.data.get("devices") != Some(&normalized) {
            self.data.insert("devices".to_owned(), normalized);
            changed = true;
        }
        changed
    }
}

/// 旧内置名 → 中文名；型号映射优先于协议映射。
fn legacy_label(
    catalog: &std::collections::HashMap<String, crate::battery::models::Model>,
    model: &str,
    profile: &str,
    old: &str,
) -> Option<String> {
    use crate::battery::legacy_labels;
    if !model.is_empty() {
        for &(id, before, after) in legacy_labels::MODEL_LABELS {
            if id.eq_ignore_ascii_case(model) && before == old {
                return Some(after.to_owned());
            }
        }
    }
    if !profile.is_empty() {
        for &(id, before, after) in legacy_labels::PROFILE_LABELS {
            if id.eq_ignore_ascii_case(profile) && before == old {
                return Some(match catalog.get(model) {
                    Some(entry) => entry.label.clone(),
                    None => after.to_owned(),
                });
            }
        }
    }
    None
}

/// 空 label / label==id 的占位只在能唯一认出型号或协议时补名。
fn placeholder_label(
    catalog: &std::collections::HashMap<String, crate::battery::models::Model>,
    model: &str,
    profile: &str,
) -> Option<String> {
    catalog
        .get(model)
        .map(|entry| entry.label.clone())
        .or_else(|| profile_label(profile))
}

fn profile_label(profile: &str) -> Option<String> {
    if profile.is_empty() {
        return None;
    }
    crate::battery::legacy_labels::PROFILE_LABELS
        .iter()
        .find(|(id, _, _)| id.eq_ignore_ascii_case(profile))
        .map(|(_, _, after)| (*after).to_owned())
}

/// 设备的硬件墓碑键：蓝牙用 `bt:ADDR`，XInput 用 `xinput:N`，HID 用 `VID:PID`。
pub fn hardware_key_of(dev: &Value) -> String {
    let bt = text_or_empty(dev.get("bt_addr")).trim().to_uppercase();
    if !bt.is_empty() {
        return format!("bt:{bt}");
    }
    let vid = text_or_empty(dev.get("vid")).trim().to_uppercase();
    let pid = text_or_empty(dev.get("pid")).trim().to_uppercase();
    if vid == "XINPUT" {
        return format!("xinput:{pid}");
    }
    let model_text = text_or_empty(dev.get("model"));
    let instance_text = text_or_empty(dev.get("instance"));
    let model = model_text.trim();
    let instance = instance_text.trim();
    if !vid.is_empty() {
        if !instance.is_empty() {
            return format!("hid:{instance}:{model}");
        }
        if !model.is_empty() {
            return format!("{vid}:{pid}:{model}");
        }
        return format!("{vid}:{pid}");
    }
    format!("id:{}", text_or_empty(dev.get("id")))
}

/// 补齐 `id`/`key`/`label`/`vid`/`pid`/`wired_pid`/`profile`/`bt_addr`/`icon`/
/// `color`/`link`/`colors`，兼容旧配置。非对象返回 `None`（V16 返回 None）。
pub fn normalize_device(dev: &Value, index: usize, used: &mut HashSet<String>) -> Option<Value> {
    let Value::Object(raw) = dev else {
        return None;
    };
    let mut device = raw.clone();

    let id = match first_truthy(raw.get("id"), raw.get("key")) {
        Some(value) => py_str(value).trim().to_owned(),
        None => String::new(),
    };
    let id = if id.is_empty() || used.contains(&id) {
        slug(raw.get("vid"), raw.get("pid"), used)
    } else {
        id
    };
    device.insert("id".to_owned(), Value::String(id.clone()));
    for (key, default) in [
        ("key", id.clone()),
        ("label", id.clone()),
        ("vid", String::new()),
        ("pid", String::new()),
        ("wired_pid", String::new()),
        ("profile", String::new()),
        ("bt_addr", String::new()),
    ] {
        if !device.contains_key(key) {
            device.insert(key.to_owned(), Value::String(default));
        }
    }

    let icon = match raw.get("icon") {
        Some(value) if truthy_of(value) => py_str(value).to_lowercase(),
        _ => String::new(),
    };
    let icon = if DEVICE_ICONS.contains(&icon.as_str()) {
        icon
    } else {
        String::new()
    };
    device.insert("icon".to_owned(), Value::String(icon));

    if !raw.get("color").is_some_and(truthy_of) {
        // 强调色按设备顺序轮转，保证新加的几台颜色不撞。
        device.insert(
            "color".to_owned(),
            Value::String(PALETTE[index % PALETTE.len()].to_owned()),
        );
    }
    if !raw.get("link").is_some_and(truthy_of) {
        device.insert("link".to_owned(), Value::String("2.4G".to_owned()));
    }

    // 只留用户真配过的那几档：没配的档位跟随全局 band_colors。
    // 早先版本会把三档一律填成出厂色，等于每台设备都「配过」，全局色就再也改不动了。
    let source = raw.get("colors").and_then(Value::as_object);
    let mut kept = Map::new();
    for band in BANDS {
        let color = match source.and_then(|map| map.get(band)) {
            Some(value) if truthy_of(value) => py_str(value).to_lowercase(),
            _ => String::new(),
        };
        if PALETTE.contains(&color.as_str()) && color != factory_color(band) {
            kept.insert(band.to_owned(), Value::String(color));
        }
    }
    device.insert("colors".to_owned(), Value::Object(kept));

    used.insert(id);
    Some(Value::Object(device))
}

fn slug(vid: Option<&Value>, pid: Option<&Value>, used: &HashSet<String>) -> String {
    let joined = format!(
        "{}{}",
        vid.map(py_str).unwrap_or_else(|| "None".to_owned()),
        pid.map(py_str).unwrap_or_else(|| "None".to_owned()),
    );
    let base: String = joined
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        .collect();
    let base = if base.is_empty() {
        "dev".to_owned()
    } else {
        base
    };
    let mut key = base.clone();
    let mut n = 2;
    while used.contains(&key) {
        key = format!("{base}-{n}");
        n += 1;
    }
    key
}

fn factory_color(band: &str) -> &'static str {
    BAND_FACTORY
        .iter()
        .find(|(name, _)| *name == band)
        .map(|(_, color)| *color)
        .unwrap_or_default()
}

fn first_truthy<'a>(a: Option<&'a Value>, b: Option<&'a Value>) -> Option<&'a Value> {
    a.filter(|value| truthy_of(value))
        .or_else(|| b.filter(|value| truthy_of(value)))
}

fn truthy(value: Option<&Value>) -> bool {
    value.is_some_and(truthy_of)
}

/// Python 真值：0 / 空串 / 空容器 / null / false 为假，`"false"` 这种非空串为真。
fn truthy_of(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_none_or(|number| number != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

fn scale_num(value: Option<&Value>) -> f64 {
    let parsed = match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.trim().parse::<f64>().ok(),
        _ => None,
    };
    parsed.filter(|number| number.is_finite()).unwrap_or(1.0)
}

fn num(value: Option<&Value>, default: f64) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(default),
        Some(Value::Bool(flag)) => {
            if *flag {
                1.0
            } else {
                0.0
            }
        }
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or(default),
        _ => default,
    }
}

fn int(value: Option<&Value>, default: i64) -> i64 {
    value.and_then(try_int).unwrap_or(default)
}

fn try_int(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Value::Bool(flag) => Some(i64::from(*flag)),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// Python `str()`（只用于拼 id / slug，正常配置里都是字符串）。
fn py_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::Number(number) => number.to_string(),
        other => other.to_string(),
    }
}

fn text_or_empty(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => py_str(other),
    }
}

/// Python `os.path.basename`：Windows 上 `/` 和 `\` 都是分隔符，末尾分隔符给出空串。
fn basename(path: &str) -> &str {
    match path.rfind(['\\', '/']) {
        Some(index) => &path[index + 1..],
        None => path,
    }
}

/// Python `max(lo, min(hi, v))`：NaN 会落到 `hi`，所以这里故意不用 `f64::clamp`。
fn clamp_f(value: f64, lo: f64, hi: f64) -> f64 {
    value.min(hi).max(lo)
}

fn clamp_i(value: i64, lo: i64, hi: i64) -> i64 {
    value.clamp(lo, hi)
}

fn to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Python `round(x, 3)`（half-even 上极端小数会差一个末位）。
fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn with_tmp_suffix(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_config(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("vxe-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir.join("config.json")
    }

    #[test]
    fn migrates_legacy_labels_once() {
        let path = temp_config("labels");
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({
                "devices": [
                    {
                        "id": "d1",
                        "label": "ATK A9 Air 大师+ / CompX 鼠标",
                        "profile": "atk-a9-air",
                        "model": "atk-a9-air"
                    },
                    {
                        "id": "d2",
                        "label": "我的鼠标",
                        "profile": "atk-a9-air",
                        "model": "atk-a9-air"
                    },
                    {
                        "id": "d3",
                        "label": "ATK A9 Air 大师+ / CompX 鼠标",
                        "profile": "atk-a9-air",
                        "bt_addr": "AA:BB:CC:DD:EE:FF"
                    },
                    {
                        "id": "d4",
                        "label": "",
                        "profile": "atk-f1-family",
                        "model": "atk-f1-ultimate"
                    }
                ],
                "card_order": ["d1"]
            }))
            .unwrap(),
        )
        .unwrap();

        let mut store = Store::load_at(&path);
        assert!(store.migrate_device_labels().unwrap());
        let saved: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let devices = saved["devices"].as_array().unwrap();
        assert_eq!(devices[0]["label"], "艾泰克 ATK A9 Air 大师+ 鼠标");
        assert_eq!(devices[1]["label"], "我的鼠标");
        assert_eq!(devices[2]["label"], "ATK A9 Air 大师+ / CompX 鼠标");
        assert_eq!(devices[3]["label"], "艾泰克 ATK F1 Ultimate 鼠标");
        assert_eq!(saved["card_order"], json!(["d1"]));
        assert!(path.with_file_name("config-migration.log").exists());
        let dir = path.parent().unwrap().to_path_buf();
        let backups = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.json.bak-")
            })
            .count();
        assert_eq!(backups, 1);

        assert!(!store.migrate_device_labels().unwrap());
        let backups_after = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.json.bak-")
            })
            .count();
        assert_eq!(backups_after, 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
