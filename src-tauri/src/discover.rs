use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::battery::{self, BatteryState, DeviceSpec, hid, models, profiles};
use crate::config;

/// 一次扫描里选出的协议；`ambiguous` = 同 VID/PID 多品牌且型号码没读出来，
/// 协议仍可用，但名字不能冒用某个品牌。
struct ProfilePick<'a> {
    profile: &'a profiles::Profile,
    ambiguous: bool,
    model_code: Option<u16>,
}

/// 同 VID/PID 被多颗公版接收器共用时（`3554:FA09` 的凌豹 K98 / 狼蛛 F99 /
/// 迈从 G75 / LEOBOG），先按会话 ACK 型号码分流；读不到型号就退回中性命名。
fn cross_vendor(exact: &[&profiles::Profile]) -> bool {
    exact
        .first()
        .is_some_and(|first| exact.iter().any(|profile| profile.vendor != first.vendor))
}

fn pick_profile<'a>(
    device: &hid::HidDevice,
    table: &'a std::collections::HashMap<String, profiles::Profile>,
) -> Option<ProfilePick<'a>> {
    let exact = profiles::exact_matches(&device.vid, &device.pid, "", table);
    if exact.len() > 1 && exact.iter().any(|profile| profile.model_code.is_some()) {
        let code = model_code_for(&device.vid, &device.pid, &device.instance);
        if let Some(model) = code
            && let Some(profile) = exact
                .iter()
                .copied()
                .find(|profile| profile.model_code == Some(model))
        {
            return Some(ProfilePick {
                profile,
                ambiguous: false,
                model_code: Some(model),
            });
        }
        return profiles::for_ids(&device.vid, &device.pid, "", table).map(|profile| ProfilePick {
            profile,
            ambiguous: true,
            model_code: code,
        });
    }
    // 同 VID/PID 跨品牌但无型号码：协议相同也只用中性名，不冒用某个品牌。
    if exact.len() > 1 && cross_vendor(&exact) {
        let code = model_code_for(&device.vid, &device.pid, &device.instance);
        return profiles::for_ids(&device.vid, &device.pid, "", table).map(|profile| ProfilePick {
            profile,
            ambiguous: true,
            model_code: code,
        });
    }
    profiles::for_ids(&device.vid, &device.pid, "", table).map(|profile| ProfilePick {
        profile,
        ambiguous: false,
        model_code: None,
    })
}

fn picked_label(pick: &ProfilePick<'_>, device: &hid::HidDevice) -> String {
    if pick.ambiguous {
        let link = if device.is_bluetooth {
            "蓝牙"
        } else {
            "2.4G"
        };
        return format!("{}:{} {} 键盘", device.vid, device.pid, link);
    }
    let model = model_label(&pick.profile.label);
    if !model.is_empty() {
        model
    } else if device.product.is_empty() {
        format!("{}:{}", device.vid, device.pid)
    } else {
        device.product.clone()
    }
}

/// 旧配置可能把「VID:PID 2.4G 键盘」这类中性标识存成名称。
fn neutral_label(label: &str, vid: &str, pid: &str) -> bool {
    let prefix = format!("{vid}:{pid}").to_lowercase();
    let trimmed = label.trim().to_lowercase();
    if prefix.len() <= 3 || !trimmed.starts_with(&prefix) {
        return false;
    }
    let rest = trimmed[prefix.len()..].trim();
    rest.is_empty()
        || [
            "2.4g", "蓝牙", "有线", "usb", "hid", "键盘", "鼠标", "耳机", "手柄",
        ]
        .iter()
        .any(|word| rest.starts_with(word))
}

/// 有已配置设备时谁的名字优先：手动名总是保留；中性名让位给醒后的真实型号。
fn known_label_wins(
    known: Option<&DeviceSpec>,
    detected: &str,
    vid: &str,
    pid: &str,
    ambiguous: bool,
) -> bool {
    let Some(spec) = known else {
        return false;
    };
    if ambiguous {
        return true;
    }
    !(neutral_label(&spec.label, vid, pid) && !neutral_label(detected, vid, pid))
}

/// 扫描结果与已配置设备匹配：唯一硬件（无线 PID 或有线 PID）直接复用配置；
/// 同硬件配置了多台时，只有型号明确且协议一致才复用。
fn configured_match<'a>(
    vid: &str,
    pid: &str,
    profile_id: &str,
    ambiguous: bool,
    instance: &str,
    model: &str,
    known: &'a [DeviceSpec],
) -> Option<&'a DeviceSpec> {
    let matches: Vec<&DeviceSpec> = known
        .iter()
        .filter(|spec| {
            spec.vid.eq_ignore_ascii_case(vid)
                && (spec.pid.eq_ignore_ascii_case(pid) || spec.wired_pid.eq_ignore_ascii_case(pid))
        })
        .collect();
    if let Some(spec) = matches.iter().copied().find(|spec| {
        !instance.is_empty()
            && spec.instance.eq_ignore_ascii_case(instance)
            && (model.is_empty() || spec.model.eq_ignore_ascii_case(model))
    }) {
        return Some(spec);
    }
    if let Some(spec) = matches.iter().copied().find(|spec| {
        !model.is_empty()
            && spec.model.eq_ignore_ascii_case(model)
            && (instance.is_empty() || spec.instance.is_empty())
    }) {
        return Some(spec);
    }
    match matches.as_slice() {
        [single] => {
            if instance.is_empty()
                || single.instance.is_empty()
                || single.instance.eq_ignore_ascii_case(instance)
            {
                Some(*single)
            } else {
                None
            }
        }
        _ if !ambiguous => matches.iter().copied().find(|spec| {
            spec.profile.eq_ignore_ascii_case(profile_id)
                && (instance.is_empty() || spec.instance.is_empty())
        }),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub id: String,
    pub label: String,
    pub vid: String,
    pub pid: String,
    pub wired_pid: String,
    pub link: String,
    pub bt_addr: String,
    pub icon: String,
    pub color: String,
    pub profile: String,
    pub profile_label: String,
    /// 识别出的型号 id；空 = 共用 ID 需要用户在添加窗选择。
    pub model: String,
    pub model_label: String,
    /// 该 VID/PID 下的型号候选（添加窗下拉）。
    pub model_candidates: Vec<models::ModelChoice>,
    /// 接收器/设备实例键。
    pub instance: String,
    pub hit: bool,
    pub level: Option<u8>,
    pub charging: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileChoice {
    pub id: String,
    pub label: String,
    pub vendor: String,
    /// 搜索别名（型号/俗称），菜单搜索命中。
    pub keywords: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectResult {
    pub ok: bool,
    pub text: String,
}

/// 公版接收器型号码缓存：成功 5 分钟、失败 10 秒，避免每次扫描都等 ACK。
/// 键包含接收器实例，两个同 VID/PID 接收器不会串型号。
type ModelCodeCache = Mutex<HashMap<(String, String, String), (Option<u16>, Instant)>>;

static MODEL_CODE_CACHE: LazyLock<ModelCodeCache> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn model_code_for(vid: &str, pid: &str, instance: &str) -> Option<u16> {
    let key = (vid.to_uppercase(), pid.to_uppercase(), instance.to_string());
    if let Ok(cache) = MODEL_CODE_CACHE.lock()
        && let Some((code, at)) = cache.get(&key)
    {
        let ttl = if code.is_some() {
            Duration::from_secs(300)
        } else {
            Duration::from_secs(10)
        };
        if at.elapsed() < ttl {
            return *code;
        }
    }
    let code = hid::read_model_code_for(vid, pid, instance);
    if let Ok(mut cache) = MODEL_CODE_CACHE.lock() {
        cache.insert(key, (code, Instant::now()));
    }
    code
}

/// 扫描阶段日志：写在 exe 同级 `scan.log`，与便携目录保持一致。
fn scan_log(line: &str) {
    use std::io::Write as _;
    let dir = config::program_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("scan.log"))
    {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis())
            .unwrap_or(0);
        let _ = writeln!(file, "{stamp} {line}");
    }
}

/// 在独立线程里跑一个可能阻塞的系统枚举；超时返回默认值，绝不让扫描无限等。
fn timed<T: Default + Send + 'static>(
    label: &str,
    timeout: Duration,
    task: impl FnOnce() -> T + Send + 'static,
) -> T {
    let start = Instant::now();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(task());
    });
    match rx.recv_timeout(timeout) {
        Ok(value) => {
            scan_log(&format!("{label}: {} ms", start.elapsed().as_millis()));
            value
        }
        Err(_) => {
            scan_log(&format!(
                "{label}: timeout after {} ms",
                timeout.as_millis()
            ));
            T::default()
        }
    }
}

/// 用户主动扫描时清掉“没读到型号”的负缓存：设备刚唤醒时，上一次扫描的
/// 失败结果不应继续挡住这次重试。
fn clear_negative_model_codes() {
    if let Ok(mut cache) = MODEL_CODE_CACHE.lock() {
        cache.retain(|_, (code, _)| code.is_some());
    }
}

pub fn scan(dirs: &[PathBuf], known: &[DeviceSpec]) -> Vec<DiscoveredDevice> {
    clear_negative_model_codes();
    let scan_start = Instant::now();
    let (table, errors) = profiles::load(dirs);
    if !errors.is_empty() {
        profiles::log_errors(config::app_dir(), &errors);
    }
    let model_table = models::load(&table);
    let mut out = Vec::new();
    let mut used = HashSet::new();
    // 扫描只列设备：HID/蓝牙/手柄并行枚举，各自套硬超时；驱动卡住时宁可少列几台，
    // 也不能让添加窗一直停在「正在扫描」。耗时写 exe 同级 scan.log。
    let (hid_devices, bt_devices, xinput_devices) = std::thread::scope(|scope| {
        let hid = scope.spawn(|| {
            timed("hid", Duration::from_secs(5), || {
                hid::list_hid_detailed(true)
            })
        });
        let bt = scope.spawn(|| timed("bt", Duration::from_secs(2), battery::bt::read_all));
        let xinput =
            scope.spawn(|| timed("xinput", Duration::from_secs(1), battery::xinput::read_all));
        (
            hid.join().unwrap_or_default(),
            bt.join().unwrap_or_default(),
            xinput.join().unwrap_or_default(),
        )
    });
    for device in &hid_devices {
        if device.usage_page >= 0xFF00 {
            scan_log(&format!(
                "hid-present {}:{} usage {:04X}:{:04X}",
                device.vid, device.pid, device.usage_page, device.usage
            ));
        }
    }
    // V16 discovery: vendor pages plus the standard pages Bluetooth HID devices
    // expose (mouse/keyboard/battery); read-only fallback so BT collections appear.
    for (index, device) in hid_devices.into_iter().enumerate() {
        if device.vid.is_empty() || device.pid.is_empty() {
            continue;
        }
        if !is_hid_candidate(&device) {
            continue;
        }
        let key = format!(
            "{}:{}:{}:{}",
            device.vid, device.pid, device.is_bluetooth, device.instance
        );
        if !used.insert(key) {
            continue;
        }
        let link = if device.is_bluetooth {
            "蓝牙"
        } else {
            "2.4G"
        };
        // 只列命中协议的候选：厂商页也被相机/集线器占用，不按协议过滤会刷出一堆无关行。
        // 注意不在这里读电量——睡眠设备每条要等数秒，会把添加窗的扫描拖到几十秒。
        let pick_start = Instant::now();
        let Some(pick) = pick_profile(&device, &table) else {
            continue;
        };
        let pick_ms = pick_start.elapsed().as_millis();
        if pick_ms > 250 {
            scan_log(&format!(
                "pick {}:{} took {pick_ms} ms",
                device.vid, device.pid
            ));
        }
        let profile = pick.profile;
        let model_pick = models::pick(
            &device.vid,
            &device.pid,
            "",
            pick.model_code,
            &device.product,
            &model_table,
        );
        let auto_model = model_pick.selected;
        let auto_model_id = auto_model.map(|model| model.id.clone()).unwrap_or_default();
        // 已配置设备：按实例优先复用；实例路径变化时回退 VID:PID + 型号/协议。
        let known_spec = if device.is_bluetooth {
            None
        } else {
            configured_match(
                &device.vid,
                &device.pid,
                &profile.id,
                pick.ambiguous,
                &device.instance,
                &auto_model_id,
                known,
            )
        };
        let model_id = known_spec
            .and_then(|spec| (!spec.model.is_empty()).then(|| spec.model.clone()))
            .unwrap_or_else(|| auto_model_id.clone());
        let chosen_model = if model_id.is_empty() {
            auto_model
        } else {
            model_table.get(&model_id).or(auto_model)
        };
        let detected_label = if let Some(model) = chosen_model {
            model.label.clone()
        } else if model_pick.ambiguous() {
            format!("{}:{} {} 设备", device.vid, device.pid, link)
        } else {
            picked_label(&pick, &device)
        };
        let prefer_known = known_label_wins(
            known_spec,
            &detected_label,
            &device.vid,
            &device.pid,
            pick.ambiguous,
        );
        let label = if prefer_known {
            known_spec
                .map(|spec| spec.label.clone())
                .unwrap_or(detected_label)
        } else {
            detected_label
        };
        let profile_id = known_spec
            .and_then(|spec| (!spec.profile.is_empty()).then(|| spec.profile.clone()))
            .or_else(|| chosen_model.map(|model| model.profile.clone()))
            .unwrap_or_else(|| profile.id.clone());
        let profile_label = table
            .get(&profile_id)
            .map(|entry| entry.label.clone())
            .unwrap_or_else(|| profile.label.clone());
        let default_icon = chosen_model
            .map(|model| model.icon.clone())
            .filter(|icon| !icon.is_empty())
            .unwrap_or_else(|| {
                guess_icon(
                    device.usage_page,
                    device.usage,
                    device.is_bluetooth,
                    &profile.label,
                )
            });
        let icon = known_spec
            .and_then(|spec| (!spec.icon.is_empty()).then(|| spec.icon.clone()))
            .unwrap_or(default_icon);
        let link_value = known_spec
            .and_then(|spec| (!spec.link.is_empty()).then(|| spec.link.clone()))
            .or_else(|| {
                chosen_model
                    .map(|model| model.link.clone())
                    .filter(|link| !link.is_empty())
            })
            .unwrap_or_else(|| link.to_string());
        let base_id = match known_spec {
            Some(spec) if prefer_known || !spec.model.is_empty() => spec.id.clone(),
            _ => {
                let mut base =
                    format!("{}{}", device.vid.to_lowercase(), device.pid.to_lowercase());
                if !model_id.is_empty() {
                    base.push('-');
                    base.push_str(&model_id);
                }
                base
            }
        };
        let spec = DeviceSpec {
            id: base_id,
            key: String::new(),
            label,
            vid: device.vid.clone(),
            pid: device.pid.clone(),
            wired_pid: String::new(),
            profile: profile_id.clone(),
            link: link_value,
            bt_addr: String::new(),
            color: config::PALETTE[index % config::PALETTE.len()].to_string(),
            colors: Map::new(),
            icon: icon.clone(),
            model: model_id.clone(),
            instance: device.instance.clone(),
        };
        // 扫描只负责列设备：逐台读电量会把列表拖成几十秒。
        // 用户点「试读」再验证协议能否读到，加入后由后台刷新采集。
        out.push(DiscoveredDevice {
            id: unique_id(&spec.id, &mut used),
            label: spec.label.clone(),
            vid: spec.vid.clone(),
            pid: spec.pid.clone(),
            wired_pid: String::new(),
            link: spec.link.clone(),
            bt_addr: String::new(),
            icon: spec.icon.clone(),
            color: spec.color.clone(),
            profile: profile_id,
            profile_label,
            model: model_id.clone(),
            model_label: chosen_model
                .map(|model| model.label.clone())
                .unwrap_or_default(),
            model_candidates: model_pick
                .candidates
                .iter()
                .map(|model| models::choice(model))
                .collect(),
            instance: device.instance.clone(),
            hit: true,
            level: None,
            charging: false,
        });
    }

    for (index, device) in bt_devices.into_iter().enumerate() {
        let key = format!("bt-audio:{}", device.addr);
        if !used.insert(key) {
            continue;
        }
        let id = unique_id(&format!("bt{}", device.addr.to_lowercase()), &mut used);
        let label = if device.label.is_empty() {
            "蓝牙设备".to_string()
        } else {
            device.label.clone()
        };
        let icon = bt_icon(&label);
        out.push(DiscoveredDevice {
            id,
            label,
            vid: device.vid,
            pid: device.pid,
            wired_pid: String::new(),
            link: battery::bt::LINK.to_string(),
            bt_addr: device.addr,
            icon,
            color: config::PALETTE[index % config::PALETTE.len()].to_string(),
            profile: battery::bt::PROFILE_ID.to_string(),
            profile_label: battery::bt::LABEL.to_string(),
            model: String::new(),
            model_label: String::new(),
            model_candidates: Vec::new(),
            instance: String::new(),
            hit: true,
            level: Some(device.level),
            charging: false,
        });
    }

    for (index, device) in xinput_devices.into_iter().enumerate() {
        let key = format!("xinput:{}", device.index);
        if !used.insert(key) {
            continue;
        }
        out.push(DiscoveredDevice {
            id: format!("xinput{}", device.index),
            label: device.label,
            vid: "XINPUT".to_string(),
            pid: device.index.to_string(),
            wired_pid: String::new(),
            link: battery::xinput::LINK.to_string(),
            bt_addr: String::new(),
            icon: "gamepad".to_string(),
            color: config::PALETTE[index % config::PALETTE.len()].to_string(),
            profile: battery::xinput::PROFILE_ID.to_string(),
            profile_label: battery::xinput::LABEL.to_string(),
            model: String::new(),
            model_label: String::new(),
            model_candidates: Vec::new(),
            instance: String::new(),
            hit: true,
            level: Some(device.level),
            charging: false,
        });
    }

    out.sort_by(|a, b| {
        b.hit
            .cmp(&a.hit)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    scan_log(&format!(
        "scan total: {} ms, {} rows",
        scan_start.elapsed().as_millis(),
        out.len()
    ));
    out
}

pub fn profile_choices(dirs: &[PathBuf]) -> Vec<ProfileChoice> {
    let (table, _errors) = profiles::load(dirs);
    let mut choices = vec![ProfileChoice {
        id: String::new(),
        label: "不指定".to_string(),
        vendor: "自动".to_string(),
        keywords: String::new(),
    }];
    let mut items: Vec<_> = table.values().collect();
    items.sort_by(|a, b| a.vendor.cmp(&b.vendor).then_with(|| a.id.cmp(&b.id)));
    choices.extend(items.into_iter().map(|profile| ProfileChoice {
        id: profile.id.clone(),
        label: profile.label.clone(),
        vendor: profile.vendor.clone(),
        keywords: profile.keywords.clone(),
    }));
    choices
}

/// 全部型号（设置/添加窗的“更改型号”完整列表）。
pub fn model_choices(dirs: &[PathBuf]) -> Vec<models::ModelChoice> {
    let (profiles, _errors) = profiles::load(dirs);
    let table = models::load(&profiles);
    let mut items: Vec<models::ModelChoice> = table.values().map(models::choice).collect();
    items.sort_by(|a, b| a.vendor.cmp(&b.vendor).then_with(|| a.label.cmp(&b.label)));
    items
}

// ---------- B：智能探测（未命中协议的厂商接口） ----------

/// 未命中任何协议的厂商 HID 接口（智能探测的对象）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnknownHid {
    pub id: String,
    pub vid: String,
    pub pid: String,
    pub usage_page: u16,
    pub usage: u16,
    pub product: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeResult {
    pub ok: bool,
    pub template_id: String,
    pub level: Option<u8>,
    pub charging: bool,
    pub profile_id: String,
    pub text: String,
}

pub fn scan_unknown(dirs: &[PathBuf]) -> Vec<UnknownHid> {
    let (table, _errors) = profiles::load(dirs);
    let mut out = Vec::new();
    let mut used = HashSet::new();
    // 不读产品字符串（个别驱动会阻塞）；这里只列 VID/PID/usage，名称够用即可。
    for device in timed("unknown-hid", Duration::from_secs(5), || {
        hid::list_hid_basic(true)
    }) {
        if device.vid.is_empty() || device.pid.is_empty() || device.usage_page < 0xFF00 {
            continue;
        }
        if profiles::for_ids(&device.vid, &device.pid, "", &table).is_some() {
            continue;
        }
        let id = format!(
            "{}:{}:{:04x}:{:04x}",
            device.vid, device.pid, device.usage_page, device.usage
        );
        if !used.insert(id.clone()) {
            continue;
        }
        out.push(UnknownHid {
            id,
            vid: device.vid.clone(),
            pid: device.pid.clone(),
            usage_page: device.usage_page,
            usage: device.usage,
            product: device.product.clone(),
        });
    }
    out
}

/// 把归一化后的协议写回 schema JSON（`payload` → `payload_hex`）。
fn profile_json(profile: &profiles::Profile) -> Value {
    let mut value = serde_json::to_value(profile).unwrap_or(Value::Null);
    if let Some(request) = value.get_mut("request").and_then(Value::as_object_mut) {
        let hex = profile
            .request
            .payload
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        request.insert("payload_hex".to_string(), Value::from(hex));
        request.remove("payload");
    }
    value
}

/// 对未知接口轮试全部协议模板；命中（两次读数接近）即生成协议 JSON 落盘。
pub fn probe_unknown(dirs: &[PathBuf], unknown: &UnknownHid) -> Result<ProbeResult, String> {
    let (table, _errors) = profiles::load(dirs);
    let spec = DeviceSpec {
        id: format!(
            "auto{}{}",
            unknown.vid.to_lowercase(),
            unknown.pid.to_lowercase()
        ),
        key: String::new(),
        label: format!("{}:{}", unknown.vid, unknown.pid),
        vid: unknown.vid.clone(),
        pid: unknown.pid.clone(),
        wired_pid: String::new(),
        profile: String::new(),
        link: "2.4G".to_string(),
        bt_addr: String::new(),
        color: config::PALETTE[0].to_string(),
        colors: Map::new(),
        icon: guess_icon(unknown.usage_page, unknown.usage, false, ""),
        model: String::new(),
        instance: String::new(),
    };
    let mut ids: Vec<&String> = table.keys().collect();
    ids.sort();
    for id in ids {
        let template = &table[id];
        let mut probe = template.clone();
        probe.id = format!(
            "auto-{}-{:04x}",
            unknown.vid.to_lowercase(),
            unknown.usage_page
        );
        probe.label = format!("自动适配 {}:{}", unknown.vid, unknown.pid);
        probe.notes = format!(
            "自动探测生成；模板 {}（{}）。识别有误可删除本文件。",
            template.id, template.label
        );
        probe.source = "auto-probe".to_string();
        probe.enabled = true;
        probe.known_ids = vec![profiles::KnownId {
            vid: unknown.vid.clone(),
            pid: unknown.pid.clone(),
            wired_pid: String::new(),
        }];
        probe.match_rule = profiles::MatchRule {
            usage_page: unknown.usage_page,
            usage: Some(unknown.usage),
            require_pid_hit: true,
        };
        let first = hid::read_by_profile(&spec, &probe);
        if first.state != BatteryState::Ok {
            continue;
        }
        let Some(level) = first.level else {
            continue;
        };
        // 稳定性：隔 200ms 再读一次，电量接近才算命中（防随机字节误判）
        std::thread::sleep(std::time::Duration::from_millis(200));
        let second = hid::read_by_profile(&spec, &probe);
        let stable = second
            .level
            .map(|value| (i32::from(value) - i32::from(level)).abs() <= 8)
            .unwrap_or(false);
        if !stable {
            continue;
        }
        let dir = dirs.first().ok_or("缺少 device_profiles 目录")?;
        let path = dir.join(format!("{}.json", probe.id));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let json = profile_json(&probe);
        let text = serde_json::to_string_pretty(&json).map_err(|err| err.to_string())?;
        std::fs::write(&path, text).map_err(|err| err.to_string())?;
        return Ok(ProbeResult {
            ok: true,
            template_id: template.id.clone(),
            level: Some(level),
            charging: second.charging,
            profile_id: probe.id.clone(),
            text: format!("已适配 · 电量 {level}%（模板 {}）", template.label),
        });
    }
    Ok(ProbeResult {
        ok: false,
        template_id: String::new(),
        level: None,
        charging: false,
        profile_id: String::new(),
        text: "轮试全部协议模板未识别出电量（多为抓包类协议）".to_string(),
    })
}

pub fn detect(spec: &DeviceSpec, profile_id: &str, dirs: &[PathBuf]) -> DetectResult {
    if battery::is_audio(spec) {
        return match battery::bt::read_level(&spec.bt_addr) {
            Some(level) => DetectResult {
                ok: true,
                text: format!("读取成功 · 电量 {level}%"),
            },
            None => DetectResult {
                ok: false,
                text: "没读到该设备的电量（蓝牙耳机要先配对并连上）".to_string(),
            },
        };
    }
    let (table, _errors) = profiles::load(dirs);
    let Some(profile) = (if profile_id.is_empty() {
        profiles::for_device(spec, &table)
    } else {
        table.get(profile_id)
    }) else {
        return DetectResult {
            ok: false,
            text: "没有匹配的协议".to_string(),
        };
    };
    let (ok, text) = battery::describe(&hid::read_by_profile(spec, profile));
    DetectResult { ok, text }
}

fn unique_id(base: &str, used: &mut HashSet<String>) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    for index in 2.. {
        let candidate = format!("{base}-{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
}

/// 协议名里的括号后缀是链路说明（"VXE V3 NK (2.4G)"），HID 产品串
/// （"NK mouse NANO dongle"）也不适合当设备名；V16 优先用协议型号命名。
fn model_label(label: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u8;
    for ch in label.chars() {
        match ch {
            '(' | '（' => depth = depth.saturating_add(1),
            ')' | '）' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn is_hid_candidate(device: &hid::HidDevice) -> bool {
    device.usage_page >= 0xFF00
        || (device.is_bluetooth && matches!(device.usage_page, 0x01 | 0x84))
        || (device.vid.eq_ignore_ascii_case("03F0")
            && device.pid.eq_ignore_ascii_case("02CC")
            && device.usage_page == 0x01C0
            && device.usage == 0x0001)
}

fn guess_icon(usage_page: u16, usage: u16, bluetooth: bool, label: &str) -> String {
    // 厂商接口（0xFFxx）没有标准 usage 可判，先看协议名：凌豹 K98 / 狼蛛 F99 这类
    // 键盘共用 0xFF02 集合，只按 usage 会全部猜成鼠标。
    let low = label.to_lowercase();
    if low.contains("键盘") || low.contains("keyboard") {
        return "keyboard".to_string();
    }
    if low.contains("耳塞") || low.contains("earbuds") {
        return "earbuds".to_string();
    }
    if low.contains("耳机") || low.contains("headset") {
        return "headset".to_string();
    }
    if low.contains("手柄") || low.contains("gamepad") || low.contains("controller") {
        return "gamepad".to_string();
    }
    if low.contains("鼠标") || low.contains("mouse") {
        return "mouse".to_string();
    }
    if usage_page == 0x01 {
        match usage {
            0x02 => return "mouse".to_string(),
            0x06 => return "keyboard".to_string(),
            0x05 => return "gamepad".to_string(),
            _ => {}
        }
    }
    if bluetooth {
        "headset".to_string()
    } else {
        "mouse".to_string()
    }
}

/// 蓝牙设备按名称猜图标（耳机/手柄/键鼠/遥控）。
fn bt_icon(label: &str) -> String {
    let low = label.to_lowercase();
    if low.contains("bud") || low.contains("ear") {
        "earbuds".to_string()
    } else if low.contains("head") || low.contains("耳机") {
        "headset".to_string()
    } else if low.contains("controller")
        || low.contains("手柄")
        || low.contains("game")
        || low.contains("remote")
        || low.contains("遥控")
    {
        "gamepad".to_string()
    } else if low.contains("keyboard") || low.contains("键盘") {
        "keyboard".to_string()
    } else if low.contains("mouse") || low.contains("鼠标") {
        "mouse".to_string()
    } else {
        "headset".to_string()
    }
}

pub fn to_device_value(device: &DiscoveredDevice) -> Value {
    let mut map = Map::new();
    map.insert("id".to_string(), Value::from(device.id.clone()));
    map.insert("key".to_string(), Value::from(device.id.clone()));
    map.insert("label".to_string(), Value::from(device.label.clone()));
    map.insert("vid".to_string(), Value::from(device.vid.clone()));
    map.insert("pid".to_string(), Value::from(device.pid.clone()));
    map.insert(
        "wired_pid".to_string(),
        Value::from(device.wired_pid.clone()),
    );
    map.insert("profile".to_string(), Value::from(device.profile.clone()));
    map.insert("link".to_string(), Value::from(device.link.clone()));
    map.insert("bt_addr".to_string(), Value::from(device.bt_addr.clone()));
    map.insert("icon".to_string(), Value::from(device.icon.clone()));
    map.insert("color".to_string(), Value::from(device.color.clone()));
    map.insert("colors".to_string(), Value::Object(Map::new()));
    map.insert("model".to_string(), Value::from(device.model.clone()));
    map.insert("instance".to_string(), Value::from(device.instance.clone()));
    Value::Object(map)
}
