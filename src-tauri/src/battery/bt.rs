//! 蓝牙耳机/音箱电量：读 Windows 自己算好的设备属性，不走 HID。
//!
//! 经典蓝牙音频设备（A2DP/HFP/AVRCP）在 HID 层根本没有接口，但系统仍然知道电量：
//! 耳机会通过 AVRCP/HFP 把它上报给蓝牙栈，栈把结果写在设备节点的一个属性上，
//! 「设置 → 蓝牙和其他设备」里那个百分比就是取的这里。我们用 cfgmgr32 读同一个属性。
//!
//! 属性键 `{104EA319-6EE2-4701-BD47-8DDBF425BBE5},2` 没有正式文档名，值是一个字节
//! （0..100，读不到就没有这个属性）；只挂在协议节点上，同一只耳机可能有 A2DP/HFP/
//! AVRCP 好几个节点，所以按蓝牙地址去重。设备名走注册表里的 FriendlyName——同一个
//! 属性用属性 API 读不出来，只有注册表有。
//!
//! 移植自 V16 `src/btaudio.py`，逐条对齐其可观察行为；所有 FFI 收口在本模块，只读，
//! 不写注册表、不修改设备状态。

use std::collections::HashMap;
use std::fmt::Write as _;
use std::mem::size_of;

use windows::Win32::Devices::Bluetooth::{
    BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BluetoothFindDeviceClose,
    BluetoothFindFirstDevice, BluetoothFindNextDevice,
};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GETIDLIST_FILTER_ENUMERATOR, CM_Get_DevNode_PropertyW, CM_Get_Device_ID_ListW,
    CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW, CR_BUFFER_SMALL, CR_SUCCESS,
};
use windows::Win32::Devices::Properties::DEVPROPTYPE;
use windows::Win32::Foundation::{DEVPROPKEY, ERROR_NO_MORE_ITEMS, TRUE};
use windows::core::{Error, GUID, HRESULT, PCWSTR};
use winreg::RegKey;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE};

pub const PROFILE_ID: &str = "bt-audio";
pub const LINK: &str = "蓝牙";
pub const LABEL: &str = "Windows 蓝牙电量";

/// `CM_Get_Device_ID_ListW` 的 ulFlags。V16 里写作 `CM_PRESENT`，值就是 1，而在
/// cfgmgr32 里 1 是 `CM_GETIDLIST_FILTER_ENUMERATOR`（pszFilter 按枚举器名解释）。
/// 保持同一个值，行为才和 V16 一致。
const CM_PRESENT: u32 = CM_GETIDLIST_FILTER_ENUMERATOR;

/// 蓝牙协议节点上的电量属性：`{104EA319-6EE2-4701-BD47-8DDBF425BBE5}, 2`。
const KEY_BATTERY: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID {
        data1: 0x104E_A319,
        data2: 0x6EE2,
        data3: 0x4701,
        data4: [0xBD, 0x47, 0x8D, 0xDB, 0xF4, 0x25, 0xBB, 0xE5],
    },
    pid: 2,
};

const ENUM_KEY: &str = r"SYSTEM\CurrentControlSet\Enum";
const FRIENDLY_NAME: &str = "FriendlyName";

/// 设备枚举器的过滤前缀，顺序即输出顺序（V16 同样先 BTHENUM 后 BTHHFENUM）。
/// `BTHLE*` 覆盖 BLE HID（T1 遥控器等）——电量属性与音频同键，只是节点不同。
const ENUM_PREFIXES: [&str; 4] = ["BTHENUM", "BTHHFENUM", "BTHLE", "BTHLEDEVICE"];

/// 协议节点的名字都拖着一串后缀，留用户认得出的那半截。
const STRIP_SUFFIXES: [&str; 5] = [
    " hands-free ag",
    " hands-free",
    " avrcp 传输",
    " stereo",
    " peripheral",
];

/// 协议节点首次查询用的缓冲区大小（u16 单位），不够就翻倍，最多 5 次。
const LIST_CAP_START: usize = 4096;
const LIST_ATTEMPTS: usize = 5;

/// 电量属性首次查询的缓冲区大小，`CM_Get_DevNode_PropertyW` 报不够就按实际值再来一次。
const PROP_CAP_START: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtDevice {
    pub addr: String,
    pub label: String,
    pub vid: String,
    pub pid: String,
    pub level: u8,
    pub instance: String,
}

/// `[{addr,label,vid,pid,level,instance}]`，只收真上报了电量的蓝牙音频设备。
///
/// 节点在位的还会再问一次蓝牙栈：已配对但链路断开的（耳机放回仓里）跳过——那个电量
/// 只是上次的缓存，报出来会被当成「已连接」。
pub fn read_all() -> Vec<BtDevice> {
    let conn = connection_map();
    let mut groups: Vec<(String, Group)> = Vec::new();
    for instance in present_ids() {
        let Some(addr) = addr_of(&instance) else {
            continue;
        };
        let index = match groups.iter().position(|(known, _)| *known == addr) {
            Some(index) => index,
            None => {
                groups.push((addr, Group::default()));
                groups.len() - 1
            }
        };
        let group = &mut groups[index].1;
        if group.level.is_none()
            && let Some(level) = battery_of(&instance)
        {
            group.level = Some(level);
            group.node = instance.clone();
        }
        group.instances.push(instance);
    }

    let mut out = Vec::new();
    for (addr, group) in groups {
        let Some(level) = group.level else {
            continue;
        };
        if conn
            .as_ref()
            .is_some_and(|map| map.get(&addr) == Some(&false))
        {
            continue;
        }
        let low = group.node.to_lowercase();
        out.push(BtDevice {
            addr,
            label: pick_name(&group.instances),
            vid: vid_of(&low).unwrap_or_default(),
            pid: pid_of(&low).unwrap_or_default(),
            level,
            instance: group.node,
        });
    }
    out
}

/// 按蓝牙地址取那一只，读不到返回 `None`。
pub fn find(addr: &str) -> Option<BtDevice> {
    let want = normalize_addr(addr);
    if want.is_empty() {
        return None;
    }
    read_all().into_iter().find(|item| item.addr == want)
}

pub fn read_level(addr: &str) -> Option<u8> {
    find(addr).map(|device| device.level)
}

/// `{蓝牙地址: 当前是否连着}`；枚举不了（没蓝牙 / API 不可用）返回 `None`。
///
/// 耳机放回仓里时，协议节点仍然「在位」，电量属性还是上一次上报的缓存值，Windows
/// 「设置」里那个百分比也是这么来的。要判断是不是真连着，只能问蓝牙栈：已配对设备的
/// `BLUETOOTH_DEVICE_INFO.fConnected` 才是当前链路状态。
pub fn connection_map() -> Option<HashMap<String, bool>> {
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: TRUE,
        fReturnRemembered: TRUE,
        fReturnConnected: TRUE,
        ..Default::default()
    };
    let mut info = BLUETOOTH_DEVICE_INFO {
        dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        ..Default::default()
    };

    // SAFETY: `params`/`info` are fully initialized (dwSize included, as the API requires)
    // and outlive the call; `info` is a valid out-parameter. `find` is closed once below.
    let find = match unsafe { BluetoothFindFirstDevice(&params, &mut info) } {
        Ok(handle) => handle,
        Err(err) if is_no_more_items(&err) => return Some(HashMap::new()),
        Err(_) => return None,
    };

    let mut out = HashMap::new();
    loop {
        // SAFETY: `info` was filled in by the call above or by the previous iteration,
        // so its bytes are readable; `rgBytes` is the 6-byte view of the address union.
        let raw = unsafe { info.Address.Anonymous.rgBytes };
        out.insert(addr_text(&raw), info.fConnected.as_bool());
        // The API requires dwSize to be refreshed before every call.
        info.dwSize = size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
        // SAFETY: `find` is a live find handle owned by this function and `info` is a
        // valid out-parameter whose dwSize was just refreshed.
        if unsafe { BluetoothFindNextDevice(find, &mut info) }.is_err() {
            break;
        }
    }
    // SAFETY: `find` came from `BluetoothFindFirstDevice` and is closed exactly once here.
    let _ = unsafe { BluetoothFindDeviceClose(find) };
    Some(out)
}

#[derive(Default)]
struct Group {
    instances: Vec<String>,
    level: Option<u8>,
    /// 真正报出电量的那个节点，vid/pid 从它这里取。
    node: String,
}

/// `re.error == ERROR_NO_MORE_ITEMS` 时 V16 把「一个都没枚举到」当成空表。
fn is_no_more_items(err: &Error) -> bool {
    err.code() == HRESULT::from_win32(ERROR_NO_MORE_ITEMS.0)
}

/// 当前在位的蓝牙设备节点实例 ID（V16 没用 ListSize 导出，缓冲区不够就加倍重试）。
///
/// 返回的是「双 \0 结尾」的多字符串，得整块拆开。
fn present_ids() -> Vec<String> {
    let mut out = Vec::new();
    for prefix in ENUM_PREFIXES {
        let filter = wide(prefix);
        let mut cap = LIST_CAP_START;
        for _ in 0..LIST_ATTEMPTS {
            let mut buf = vec![0u16; cap];
            // SAFETY: `filter` is a NUL-terminated UTF-16 string that outlives the call and
            // `buf` is a writable buffer of exactly `buf.len()` code units.
            let rc =
                unsafe { CM_Get_Device_ID_ListW(PCWSTR(filter.as_ptr()), &mut buf, CM_PRESENT) };
            if rc == CR_SUCCESS {
                out.extend(multi_sz(&buf));
                break;
            }
            if rc != CR_BUFFER_SMALL {
                break;
            }
            cap *= 2;
        }
    }
    out
}

/// 拆「双 \0 结尾」的多字符串块，丢掉空串（和 V16 的 `split("\0")` + `if s` 等价）。
fn multi_sz(buf: &[u16]) -> Vec<String> {
    buf.split(|&unit| unit == 0)
        .filter(|part| !part.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

/// 读一个设备属性，返回 (类型, bytes)；读不到返回 `None`。
///
/// 类型本身用不上（不同蓝牙栈给同一个值标的类型不一样，V16 实测是 INT16），按原始
/// 字节解释，这里保留它只是为了和 V16 的 `_prop` 对齐。
fn devnode_property(devinst: u32, key: &DEVPROPKEY, cap: usize) -> Option<(DEVPROPTYPE, Vec<u8>)> {
    let mut cap = cap.max(1);
    for _ in 0..2 {
        let mut ptype = DEVPROPTYPE(0);
        let mut size = u32::try_from(cap).ok()?;
        let mut buf = vec![0u8; cap];
        // SAFETY: `key` is a valid DEVPROPKEY, `ptype`/`size` are valid out-parameters and
        // `buf` is writable for `size` bytes, which is what `size` reports as its capacity.
        let rc = unsafe {
            CM_Get_DevNode_PropertyW(
                devinst,
                key,
                &mut ptype,
                Some(buf.as_mut_ptr()),
                &mut size,
                0,
            )
        };
        if rc == CR_SUCCESS {
            // Python 切片 `buf[:size]`：size 超过容量时也只给整块。
            buf.truncate((size as usize).min(buf.len()));
            return Some((ptype, buf));
        }
        if rc == CR_BUFFER_SMALL && size > 0 {
            cap = size as usize;
        } else {
            return None;
        }
    }
    None
}

fn battery_of(instance: &str) -> Option<u8> {
    let device_id = wide(instance);
    let mut devinst: u32 = 0;
    // SAFETY: `device_id` is a NUL-terminated UTF-16 string that outlives the call and
    // `devinst` is a valid out-parameter for one devnode id.
    let rc = unsafe {
        CM_Locate_DevNodeW(
            &mut devinst,
            PCWSTR(device_id.as_ptr()),
            CM_LOCATE_DEVNODE_NORMAL,
        )
    };
    if rc != CR_SUCCESS {
        return None;
    }
    // 值本身是一个字节，但不同栈标的类型不一样（V16 实测标的是 INT16），按原始字节取。
    let (_ptype, raw) = devnode_property(devinst, &KEY_BATTERY, PROP_CAP_START)?;
    battery_level(&raw)
}

/// 原始字节 → 电量：1..=2 字节小端，且 `<= 100` 才算有效（V16 `_battery_of` 的尾部）。
fn battery_level(raw: &[u8]) -> Option<u8> {
    let level = match raw {
        [byte] => u16::from(*byte),
        [low, high] => u16::from_le_bytes([*low, *high]),
        _ => return None,
    };
    (level <= 100).then_some(level as u8)
}

/// 注册表里的 FriendlyName；读不到返回空串。
fn reg_name(instance: &str) -> String {
    let path = format!(r"{ENUM_KEY}\{instance}");
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(&path, KEY_QUERY_VALUE)
        .and_then(|key| key.get_value::<String, _>(FRIENDLY_NAME))
        .map(|name| name.trim().to_string())
        .unwrap_or_default()
}

/// 同一地址的几个节点里挑设备名。
///
/// FriendlyName 常常是 `@dll,#1;%1 Hands-Free AG%0;(Bluetooth Headset)` 这种间接串，
/// 我们解不了资源；但尾巴上括号里那份就是 Windows 显示的型号，直接取它。
fn pick_name(instances: &[String]) -> String {
    for instance in instances {
        let text = reg_name(instance);
        if !text.is_empty() && !text.starts_with('@') {
            return clean_name(&text);
        }
    }
    for instance in instances {
        let text = reg_name(instance);
        if let Some(hint) = trailing_hint(&text) {
            let hint = hint.trim();
            if !hint.is_empty() {
                return clean_name(hint);
            }
        }
    }
    String::new()
}

/// 剥掉协议节点名字上的后缀；剥空了就原样返回（V16 `... .strip() or text`）。
fn clean_name(text: &str) -> String {
    let low = text.to_lowercase();
    for suffix in STRIP_SUFFIXES {
        if low.ends_with(suffix) {
            // 后缀是 ASCII，正常输入下按字节截断就是对的；万一非 ASCII 小写改变了长度，
            // `get` 会返回 None，那样也回到「原样返回」。
            let cut = text.len().saturating_sub(suffix.len());
            let trimmed = text.get(..cut).map_or("", str::trim);
            if trimmed.is_empty() {
                return text.to_string();
            }
            return trimmed.to_string();
        }
    }
    text.to_string()
}

/// V16 `_RE_HINT`：`\(([^()]*)\)\s*\Z`，`re.search` 取最左命中，返回括号里的内容。
fn trailing_hint(text: &str) -> Option<&str> {
    for (open, _) in text.match_indices('(') {
        let inner = &text[open + 1..];
        let Some(close) = inner.find(')') else {
            continue;
        };
        if inner[..close].contains('(') {
            continue;
        }
        if !inner[close + 1..].trim_end().is_empty() {
            continue;
        }
        return Some(&inner[..close]);
    }
    None
}

/// V16 `_RE_ADDR`：实例名末尾 12 位十六进制 MAC，可带 `_hex` 后缀，取最左命中。
fn addr_of(instance: &str) -> Option<String> {
    let low = instance.to_lowercase();
    let bytes = low.as_bytes();
    for (start, run) in bytes.windows(12).enumerate() {
        if !run.iter().all(|byte| is_hex(*byte)) {
            continue;
        }
        let rest = &bytes[start + 12..];
        let tail_ok = rest.is_empty()
            || (rest[0] == b'_' && rest.len() > 1 && rest[1..].iter().all(|byte| is_hex(*byte)));
        if tail_ok {
            return Some(String::from_utf8_lossy(run).to_uppercase());
        }
    }
    None
}

/// V16 `_RE_VID`：`vid&[0-9a-f]{4}([0-9a-f]{4})`，取后 4 位，大写。
fn vid_of(instance_lower: &str) -> Option<String> {
    let run = hex_run_after(instance_lower, b"vid&", 8)?;
    Some(run[4..8].to_uppercase())
}

/// V16 `_RE_PID`：`pid&([0-9a-f]{4,8})`，取最后 4 位，大写。
fn pid_of(instance_lower: &str) -> Option<String> {
    let run = hex_run_after(instance_lower, b"pid&", 4)?;
    // `{4,8}` 是贪婪的：超过 8 位只吃前 8 位。
    let run = &run[..run.len().min(8)];
    Some(run[run.len() - 4..].to_uppercase())
}

/// 在 `text` 里找最左的 `tag`，返回其后连续的小写十六进制串（长度至少 `min`）。
fn hex_run_after<'a>(text: &'a str, tag: &[u8], min: usize) -> Option<&'a str> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len().saturating_sub(tag.len() - 1) {
        if &bytes[start..start + tag.len()] != tag {
            continue;
        }
        let from = start + tag.len();
        let mut to = from;
        while to < bytes.len() && is_hex(bytes[to]) {
            to += 1;
        }
        if to - from >= min {
            // 全是 ASCII 十六进制，字节下标必然落在字符边界上。
            return Some(&text[from..to]);
        }
    }
    None
}

fn is_hex(byte: u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'a'..=b'f')
}

/// V16 `_addr_text`：`Address.rgBytes` 反序，每字节两字符大写十六进制。
fn addr_text(raw: &[u8; 6]) -> String {
    let mut out = String::with_capacity(12);
    for byte in raw.iter().rev() {
        // 写 String 不会失败。
        let _ = write!(out, "{byte:02X}");
    }
    out
}

/// V16 `find` 的输入规范化：去掉 `:`/`-`、去首尾空白、转大写。
pub fn normalize_addr(addr: &str) -> String {
    addr.replace([':', '-'], "").trim().to_uppercase()
}

/// UTF-16 + 结尾 NUL，供 `PCWSTR` 参数使用。
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
