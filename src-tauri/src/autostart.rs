#![allow(dead_code)] // 注册表读写只在 release 构建调用；debug 构建不触碰注册表。

use std::io;
use std::path::Path;
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const VALUE_NAME: &str = "VXE";
/// 旧产品名的 Run 值，启动时删除，避免继续指向已不存在的旧 exe。
pub const LEGACY_VALUE_NAME: &str = "设备电量";
pub const AUTOSTART_ARG: &str = "--tray";

pub fn command(exe_path: &Path) -> String {
    format!("\"{}\" {}", exe_path.display(), AUTOSTART_ARG)
}

fn run_key(access: u32) -> io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, access)
}

pub fn ensure(exe_path: &Path) -> io::Result<()> {
    let key = run_key(KEY_SET_VALUE)?;
    key.set_value(VALUE_NAME, &command(exe_path))
}

fn remove_named(name: &str) -> io::Result<()> {
    let key = match run_key(KEY_SET_VALUE) {
        Ok(key) => key,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    match key.delete_value(name) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

pub fn remove() -> io::Result<()> {
    remove_named(VALUE_NAME)
}

pub fn sync(enabled: bool, exe_path: &Path) -> io::Result<()> {
    if enabled { ensure(exe_path) } else { remove() }
}

/// 改名迁移：先删旧产品名的自启值，再按当前开关写入 VXE。
pub fn migrate(enabled: bool, exe_path: &Path) -> io::Result<()> {
    remove_named(LEGACY_VALUE_NAME)?;
    sync(enabled, exe_path)
}
