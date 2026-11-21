//! XInput 手柄电量（Xbox 手柄等）。
//!
//! XInput 的 `XInputGetBatteryInformation` 只给 4 档（空/低/中/满），
//! 按 GearPower 惯例映射为 5/20/50/100。手柄无名字，按槽位命名。

use windows::Win32::UI::Input::XboxController::{
    BATTERY_DEVTYPE, XINPUT_BATTERY_INFORMATION, XINPUT_STATE, XInputGetBatteryInformation,
    XInputGetState,
};

pub const PROFILE_ID: &str = "xinput";
pub const LINK: &str = "手柄";
pub const LABEL: &str = "XInput 手柄电量";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadDevice {
    pub index: u32,
    pub label: String,
    pub level: u8,
}

fn level_of(raw: u32) -> Option<u8> {
    // BATTERY_LEVEL_* 是 i32 枚举；0..3 = 空/低/中/满
    match raw {
        0 => Some(5),   // EMPTY
        1 => Some(20),  // LOW
        2 => Some(50),  // MEDIUM
        3 => Some(100), // FULL
        _ => None,
    }
}

/// 四个 XInput 槽位里在线且读到电量的手柄。
pub fn read_all() -> Vec<PadDevice> {
    let mut out = Vec::new();
    for index in 0..4u32 {
        let Some(level) = read_level(index) else {
            continue;
        };
        out.push(PadDevice {
            index,
            label: format!("Xbox 手柄 {}", index + 1),
            level,
        });
    }
    out
}

/// 单槽位电量；手柄不在线或电量未知返回 `None`。
pub fn read_level(index: u32) -> Option<u8> {
    if index > 3 {
        return None;
    }
    unsafe {
        let mut state = XINPUT_STATE::default();
        // 返回 Win32 错误码：0 = 成功（手柄在线）
        if XInputGetState(index, &mut state) != 0 {
            return None;
        }
        let mut info = XINPUT_BATTERY_INFORMATION::default();
        // devType 0 = BATTERY_DEVTYPE（手柄）
        if XInputGetBatteryInformation(index, BATTERY_DEVTYPE(0), &mut info) != 0 {
            return None;
        }
        // 档位 0..3 = 空/低/中/满；255（UNKNOWN）等落 None。
        level_of(info.BatteryLevel.0 as u32)
    }
}
