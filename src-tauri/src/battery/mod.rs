pub mod bt;
pub mod decode;
pub mod flydigi;
pub mod hid;
pub mod legacy_labels;
pub mod models;
pub mod passive;
pub mod profiles;
pub mod xinput;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BatteryState {
    Ok,
    Sleep,
    Absent,
    #[default]
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DeviceSpec {
    pub id: String,
    pub key: String,
    pub label: String,
    pub vid: String,
    pub pid: String,
    pub wired_pid: String,
    pub profile: String,
    pub link: String,
    pub bt_addr: String,
    pub color: String,
    pub colors: Map<String, Value>,
    pub icon: String,
    /// 型号目录 id（身份）；空 = 旧配置/未识别。
    pub model: String,
    /// 接收器/设备实例键（同 VID:PID 的两颗接收器靠它分开）。
    pub instance: String,
}

impl DeviceSpec {
    pub fn from_json(value: &Value) -> Self {
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string()
        };
        let id = text("id");
        let key = text("key");
        let colors = value
            .get("colors")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        Self {
            id: if id.is_empty() { key.clone() } else { id },
            key,
            label: text("label"),
            vid: text("vid"),
            pid: text("pid"),
            wired_pid: text("wired_pid"),
            profile: text("profile"),
            link: if text("link").is_empty() {
                "2.4G".to_string()
            } else {
                text("link")
            },
            bt_addr: text("bt_addr"),
            color: if text("color").is_empty() {
                "blue".to_string()
            } else {
                text("color")
            },
            colors,
            icon: text("icon"),
            model: text("model"),
            instance: text("instance"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub id: String,
    pub label: String,
    pub vid: String,
    pub pid: String,
    pub state: BatteryState,
    pub level: Option<u8>,
    pub charging: bool,
    pub link: String,
    pub color: String,
    pub colors: Map<String, Value>,
    pub profile: String,
    pub icon: String,
    pub last: Option<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct ReadResult {
    pub state: BatteryState,
    pub level: Option<u8>,
    pub charging: bool,
}

pub fn is_audio(dev: &DeviceSpec) -> bool {
    dev.profile.eq_ignore_ascii_case(bt::PROFILE_ID) || dev.link == bt::LINK
}

pub fn describe(result: &ReadResult) -> (bool, String) {
    match (result.state, result.level) {
        (BatteryState::Ok, Some(level)) => {
            let extra = if result.charging {
                "（充电中）"
            } else {
                ""
            };
            (true, format!("读取成功 · 电量 {level}%{extra}"))
        }
        (BatteryState::Sleep, _) => (
            false,
            "设备没有应答（可能正在休眠，动一下鼠标再试）".to_string(),
        ),
        (BatteryState::Absent, _) => (
            false,
            "没读到该设备的电量（蓝牙耳机要先配对并连上）".to_string(),
        ),
        (BatteryState::Error, _) => (false, "读取失败：该设备可能不支持 HID 电量读取".to_string()),
        _ => (false, "未识别".to_string()),
    }
}

pub fn read_all(
    devices: &[DeviceSpec],
    profile_dirs: &[PathBuf],
    error_log_dir: Option<&std::path::Path>,
    passive: Option<&passive::Hub>,
) -> Vec<DeviceStatus> {
    let (table, errors) = profiles::load(profile_dirs);
    if let Some(dir) = error_log_dir {
        profiles::log_errors(dir, &errors);
    }
    if let Some(hub) = passive {
        hub.sync(devices, &table);
    }
    // 蓝牙枚举一次即可覆盖所有蓝牙设备；旧实现每台都重新枚举一遍。
    let audio_levels: HashMap<String, u8> = if devices.iter().any(is_audio) {
        bt::read_all()
            .into_iter()
            .map(|device| (bt::normalize_addr(&device.addr), device.level))
            .collect()
    } else {
        HashMap::new()
    };
    let flydigi_status = if devices.iter().any(flydigi::matches) {
        flydigi::query()
    } else {
        flydigi::Status::Unavailable
    };
    // HID/蓝牙/手柄逐台读取互不依赖，并行后刷新耗时从「累加」降到「最慢一台」。
    let infos: Vec<ReadResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = devices
            .iter()
            .map(|dev| {
                let table = &table;
                let audio = &audio_levels;
                scope.spawn(move || {
                    if flydigi::matches(dev) {
                        flydigi_status.reading()
                    } else {
                        read_one(dev, table, audio, passive)
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_default())
            .collect()
    });
    devices
        .iter()
        .zip(infos)
        .map(|(dev, info)| DeviceStatus {
            id: dev.id.clone(),
            label: dev.label.clone(),
            vid: dev.vid.clone(),
            pid: dev.pid.clone(),
            state: info.state,
            level: info.level,
            charging: info.charging,
            link: dev.link.clone(),
            color: dev.color.clone(),
            colors: dev.colors.clone(),
            profile: if dev.profile.is_empty() {
                profiles::for_device(dev, &table)
                    .map(|profile| profile.id.clone())
                    .unwrap_or_default()
            } else {
                dev.profile.clone()
            },
            icon: dev.icon.clone(),
            last: None,
        })
        .collect()
}

fn read_one(
    dev: &DeviceSpec,
    table: &HashMap<String, profiles::Profile>,
    audio_levels: &HashMap<String, u8>,
    passive: Option<&passive::Hub>,
) -> ReadResult {
    if is_audio(dev) {
        return match audio_levels.get(&bt::normalize_addr(&dev.bt_addr)).copied() {
            Some(level) => ReadResult {
                state: BatteryState::Ok,
                level: Some(level),
                charging: false,
            },
            None => ReadResult {
                state: BatteryState::Absent,
                level: None,
                charging: false,
            },
        };
    }
    if dev.profile.eq_ignore_ascii_case(xinput::PROFILE_ID) {
        let index = dev.pid.parse::<u32>().unwrap_or(u32::MAX);
        return match xinput::read_level(index) {
            Some(level) => ReadResult {
                state: BatteryState::Ok,
                level: Some(level),
                charging: false,
            },
            None => ReadResult {
                state: BatteryState::Absent,
                level: None,
                charging: false,
            },
        };
    }
    let Some(profile) = profiles::for_device(dev, table) else {
        return ReadResult {
            state: BatteryState::Error,
            level: None,
            charging: false,
        };
    };
    if profile.request.transport == profiles::Transport::Passive
        && let Some(hub) = passive
    {
        return hub.read(dev, profile);
    }
    hid::read_by_profile(dev, profile)
}
