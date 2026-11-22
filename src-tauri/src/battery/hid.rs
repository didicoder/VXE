//! HID enumeration and battery reads — port of V16 `src/hiddev.py`.
//!
//! The read logic is profile driven: [`crate::battery::profiles`] says which usage
//! page to look at, which report to send, where the level / charging bytes live and
//! how to sample. This module owns only the Win32 plumbing (SetupAPI enumeration,
//! `CreateFileW` + overlapped `WriteFile`/`ReadFile`, `HidD_GetFeature`).
//!
//! Porting notes (V16 behaviour kept on purpose):
//! * `require_pid_hit` keeps its inverted meaning: `false` tries the exact
//!   `pid`/`wired_pid` interfaces first and only then falls back to "same VID".
//! * `pick = mode_nonzero` only aggregates samples `> 0`; an all-zero sample set is
//!   reported as `sleep`, not `0%`.
//! * The whole device read runs on a detached thread with a `recv_timeout` deadline
//!   (`sampling.total_timeout_s`); timing out yields `sleep` and does not block the
//!   caller.
//! * A profile reply of exactly 0 bytes still counts as one full buffer (V16
//!   `nbytes = n or len(buf)`).
//! * `round()` uses banker's rounding (`f64::round_ties_even`), matching Python.
//!
//! Deliberate differences from V16:
//! * `HidDevice` carries `product` / `manufacturer` (V16's `list_hid` did not read
//!   strings); they are only filled by [`list_hid`], never on the read path.
//! * After `CancelIoEx` we wait (bounded) for the cancelled I/O to finish before the
//!   buffer and handle are released; V16 freed them immediately, which is a
//!   use-after-free risk flagged in `docs/WIN32-MAP.md`.
//! * `mode_nonzero` ties break to the smallest value; V16 left the tie order
//!   unspecified ("不要依赖").

use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HIDP_CAPS, HidD_FreePreparsedData, HidD_GetFeature, HidD_GetManufacturerString,
    HidD_GetPreparsedData, HidD_GetProductString, HidD_SetFeature, HidP_GetCaps,
    PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::{GUID, PCWSTR};

use crate::battery::profiles::{LevelEncoding, Pick, Profile, Transport};
use crate::battery::{BatteryState, DeviceSpec, ReadResult};

/// HID device interface class `{4D1E55B2-F16F-11CF-88CB-001111000030}`.
const HID_INTERFACE_GUID: GUID = GUID::from_u128(0x4D1E55B2_F16F_11CF_88CB_001111000030);

/// `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` — the overlapped call was accepted.
const HRESULT_ERROR_IO_PENDING: i32 = 0x8007_03E5u32 as i32;

/// V16 hard-codes `cbSize = 8` for `SP_DEVICE_INTERFACE_DETAIL_DATA_W`; that is the
/// DWORD-aligned size used by 64-bit Windows (`size_of` also yields 8 there, but the
/// constant documents the intent and keeps the 32-bit build identical to V16).
const DETAIL_DATA_CB_SIZE: u32 = 8;

/// Extra room V16 allocated beyond `requiredsize` (`create_string_buffer(req + 16)`).
const DETAIL_DATA_SLACK: usize = 16;

/// Longest device-internal I/O wait after `CancelIoEx` before giving up.
const CANCEL_WAIT_MS: u32 = 2_000;

/// `HidD_GetProductString` / `HidD_GetManufacturerString` buffer, in UTF-16 units.
const HID_STRING_CHARS: usize = 128;

/// Fallback when a profile carries a nonsense `total_timeout_s`.
const DEFAULT_TOTAL_TIMEOUT_S: f64 = 4.5;

// `CreateFileW` and `CreateEventW` live behind the `Win32_Security` feature of the
// `windows` crate, which this crate does not enable. Both are plain kernel32
// exports whose security-attribute argument is documented as optional (NULL), so
// they are declared here rather than widening the crate features.
//
// SAFETY: the declarations mirror the documented kernel32 ABI exactly (calling
// convention `system`, `HANDLE` is a pointer-sized opaque value, all pointer and
// length arguments are passed through unchanged), so calling them with the
// arguments used below is sound. Callers stay inside this module.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        lpfilename: *const u16,
        dwdesiredaccess: u32,
        dwsharemode: u32,
        lpsecurityattributes: *const core::ffi::c_void,
        dwcreationdisposition: u32,
        dwflagsandattributes: u32,
        htemplatefile: HANDLE,
    ) -> HANDLE;
    fn CreateEventW(
        lpeventattributes: *const core::ffi::c_void,
        bmanualreset: i32,
        binitialstate: i32,
        lpname: *const u16,
    ) -> HANDLE;
}

/// One HID interface as exposed to the rest of the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidDevice {
    pub path: String,
    pub vid: String,
    pub pid: String,
    pub usage_page: u16,
    pub usage: u16,
    pub product: String,
    pub manufacturer: String,
    pub is_bluetooth: bool,
    /// 接收器/设备的稳定实例键（同一接收器的多个 collection 共用一个值）。
    pub instance: String,
}

/// An interface plus the report lengths taken from `HidP_GetCaps`.
///
/// The frozen [`HidDevice`] shape has no room for them, but sampling needs them as
/// the fallback when a profile leaves `out_len` / `in_len` unset (V16 `d.in_len`).
#[derive(Debug, Clone)]
struct Interface {
    dev: HidDevice,
    in_len: u16,
    out_len: u16,
    feat_len: u16,
}

/// Owned Win32 handle; closes itself exactly once.
struct Handle(HANDLE);

impl Handle {
    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this guard and is closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

// ---------------------------------------------------------------------------
// Path parsing (V16 `_grab` / `is_bluetooth`)
// ---------------------------------------------------------------------------

/// USB paths look like `vid_045e&pid_00b1`; BTHENUM paths look like
/// `VID&0002045e_PID&02fd` — the real id is the last 4 hex digits of the 4..8 digit
/// run. `low_path` must already be lower-cased.
fn grab(low_path: &str, tag: &str) -> String {
    let bytes = low_path.as_bytes();
    let tag_bytes = tag.as_bytes();
    let mut index = 0usize;
    while index + 4 <= bytes.len() {
        let head = &bytes[index..index + 3];
        if (head == b"vid" || head == b"pid")
            && (bytes[index + 3] == b'_' || bytes[index + 3] == b'&')
        {
            let start = index + 4;
            let mut end = start;
            while end < bytes.len()
                && end - start < 8
                && matches!(bytes[end], b'0'..=b'9' | b'a'..=b'f')
            {
                end += 1;
            }
            if end - start >= 4 {
                if head == tag_bytes {
                    return low_path[end - 4..end].to_ascii_uppercase();
                }
                // The regex equivalent consumed this run; continue after it.
                index = end;
                continue;
            }
            // Fewer than four hex digits: no match at this offset.
            index += 1;
            continue;
        }
        index += 1;
    }
    String::new()
}

/// True when the device instance path says the interface arrives over Bluetooth.
fn is_bluetooth_path(low_path: &str) -> bool {
    low_path.contains("bthenum") || low_path.contains("bluetooth")
}

/// `(vid, pid, is_bluetooth)` for one interface path.
fn parse_ids(path: &str) -> (String, String, bool) {
    let low = path.to_ascii_lowercase();
    (
        grab(&low, "vid"),
        grab(&low, "pid"),
        is_bluetooth_path(&low),
    )
}

/// 同一接收器的多个 collection 共用实例键：路径里 `#8&687e681&0&0003#` 的
/// `8&687e681`；两个同 VID/PID 的接收器会得到不同的哈希。
fn instance_of(path: &str) -> String {
    let low = path.to_ascii_lowercase();
    // `\\?\hid#<interface>#<instance>#{guid}`：实例是第三段。
    let mut parts = low.split('#');
    let _device = parts.next();
    let _interface = parts.next();
    let Some(segment) = parts.next() else {
        return low;
    };
    if segment.is_empty() {
        return low;
    }
    segment
        .rsplit_once("&0&")
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_else(|| segment.to_string())
}

// ---------------------------------------------------------------------------
// SetupAPI enumeration
// ---------------------------------------------------------------------------

/// Enumerate every present HID device interface path, in SetupAPI order.
fn enum_paths() -> Vec<String> {
    let mut paths = Vec::new();

    // SAFETY: the class GUID is a valid static; enumerator and parent window are
    // documented as optional (NULL).
    let set = unsafe {
        SetupDiGetClassDevsW(
            Some(&HID_INTERFACE_GUID),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    let Ok(set) = set else {
        return paths;
    };

    let mut index = 0u32;
    loop {
        let mut data = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        // SAFETY: `set` is a live device info set; `data.cbSize` is initialized and
        // `data` is a valid out-parameter. Any error (including
        // ERROR_NO_MORE_ITEMS) ends the enumeration, exactly like V16.
        let ok = unsafe {
            SetupDiEnumDeviceInterfaces(set, None, &HID_INTERFACE_GUID, index, &mut data)
        };
        if ok.is_err() {
            break;
        }
        index += 1;
        if let Some(path) = interface_path(set, &data) {
            paths.push(path);
        }
    }

    // SAFETY: `set` came from SetupDiGetClassDevsW and is released exactly once.
    let _ = unsafe { SetupDiDestroyDeviceInfoList(set) };
    paths
}

/// Second half of the two-pass `SetupDiGetDeviceInterfaceDetailW` dance.
fn interface_path(set: HDEVINFO, data: &SP_DEVICE_INTERFACE_DATA) -> Option<String> {
    let mut required = 0u32;
    // SAFETY: first pass only asks for the required byte count; `required` is a
    // valid out-parameter and the call is expected to fail with ERROR_INSUFFICIENT_BUFFER.
    let _ =
        unsafe { SetupDiGetDeviceInterfaceDetailW(set, data, None, 0, Some(&mut required), None) };
    if required == 0 {
        return None;
    }

    // V16 allocates `required + 16` bytes. `u32` words give the 4-byte alignment the
    // detail struct requires (a `Vec<u8>` would only be 1-byte aligned).
    let words = (required as usize + DETAIL_DATA_SLACK).div_ceil(4);
    let mut buffer = vec![0u32; words];
    let detail = buffer
        .as_mut_ptr()
        .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();

    // SAFETY: `buffer` holds at least `required` bytes and is 4-byte aligned; V16
    // writes the same DWORD value before the second call.
    unsafe { (*detail).cbSize = DETAIL_DATA_CB_SIZE };

    // SAFETY: same buffer and size that the first pass reported.
    let ok = unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            set,
            data,
            Some(detail),
            required,
            Some(&mut required),
            None,
        )
    };
    if ok.is_err() {
        return None;
    }

    // SAFETY: the call filled `buffer` with a NUL-terminated UTF-16 device path that
    // starts right after the DWORD `cbSize` field.
    let wide = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u16>(), words * 2) };
    let start = 2; // two UTF-16 units == the 4-byte cbSize prefix
    let end = wide[start..]
        .iter()
        .position(|unit| *unit == 0)
        .map(|offset| start + offset)
        .unwrap_or(wide.len());
    let path = String::from_utf16_lossy(&wide[start..end]);
    (!path.is_empty()).then_some(path)
}

// ---------------------------------------------------------------------------
// Handles, caps and strings
// ---------------------------------------------------------------------------

/// `CreateFileW` with `FILE_SHARE_READ | FILE_SHARE_WRITE` and `OPEN_EXISTING`.
fn open_path(path: &str, write: bool, overlapped: bool) -> Option<Handle> {
    use std::ffi::OsStr;

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let access = if write {
        GENERIC_READ.0 | GENERIC_WRITE.0
    } else {
        GENERIC_READ.0
    };
    let flags = if overlapped {
        FILE_FLAG_OVERLAPPED.0
    } else {
        0
    };

    // SAFETY: `wide` is NUL-terminated and outlives the call; the security-attribute
    // and template-handle arguments are documented as optional (NULL). The returned
    // handle is owned by `Handle`, which only stores invalid-free values.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            access,
            FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
            ptr::null(),
            OPEN_EXISTING.0,
            flags,
            HANDLE(ptr::null_mut()),
        )
    };
    (!handle.is_invalid()).then_some(Handle(handle))
}

/// V16 `open_handle`: read/write first, read-only fallback when allowed.
fn open_interface(path: &str, read_only: bool) -> Option<Handle> {
    if let Some(handle) = open_path(path, true, false) {
        return Some(handle);
    }
    if read_only {
        open_path(path, false, false)
    } else {
        None
    }
}

/// `HidD_GetPreparsedData` → `HidP_GetCaps` → `HidD_FreePreparsedData`.
///
/// Returns `None` only when the preparsed data could not be obtained; a failing
/// `HidP_GetCaps` leaves a zeroed struct, exactly like V16's `_caps`.
fn read_caps(handle: HANDLE) -> Option<HIDP_CAPS> {
    let mut preparsed = PHIDP_PREPARSED_DATA::default();
    // SAFETY: `handle` is an open HID handle and `preparsed` is a valid out-parameter.
    if !unsafe { HidD_GetPreparsedData(handle, &mut preparsed) } {
        return None;
    }
    // SAFETY: HIDP_CAPS is a POD struct of u16 fields; zeroed is a valid value.
    let mut caps: HIDP_CAPS = unsafe { std::mem::zeroed() };
    // SAFETY: `preparsed` was produced for this handle and is still allocated.
    let _status = unsafe { HidP_GetCaps(preparsed, &mut caps) };
    // SAFETY: the preparsed data is freed exactly once here.
    let _ = unsafe { HidD_FreePreparsedData(preparsed) };
    Some(caps)
}

type HidStringFn = unsafe fn(HANDLE, *mut core::ffi::c_void, u32) -> bool;

/// Read one UTF-16 device string; missing strings become an empty `String`
/// (V16 `_hid_string`).
fn hid_string(get: HidStringFn, handle: HANDLE) -> String {
    let mut buffer = [0u16; HID_STRING_CHARS];
    // SAFETY: `buffer` is a valid `HID_STRING_CHARS * 2` byte writable buffer and the
    // length passed matches it, exactly like V16's `sizeof(create_unicode_buffer(128))`.
    let ok = unsafe {
        get(
            handle,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * size_of::<u16>()) as u32,
        )
    };
    if !ok {
        return String::new();
    }
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end]).trim().to_string()
}

/// `(product, manufacturer)` for one open handle.
fn hid_strings(handle: HANDLE) -> (String, String) {
    (
        hid_string(HidD_GetProductString, handle),
        hid_string(HidD_GetManufacturerString, handle),
    )
}

/// Enumerate HID interfaces (V16 `list_hid`).
///
/// Every interface is opened read/write first; when that fails and `read_only` is
/// set the interface is retried read-only (Bluetooth collections and standard
/// battery pages usually reject writes).
/// 扫描列表用：不读产品/厂商字符串（个别驱动会阻塞），只要 VID/PID/usage。
pub fn list_hid_basic(read_only: bool) -> Vec<HidDevice> {
    list_interfaces(0, read_only, false)
        .into_iter()
        .map(|interface| interface.dev)
        .collect()
}

/// 带产品/厂商字符串的扫描：型号目录需要产品名区分共用 VID:PID。
pub fn list_hid_detailed(read_only: bool) -> Vec<HidDevice> {
    list_interfaces(0, read_only, true)
        .into_iter()
        .map(|interface| interface.dev)
        .collect()
}

/// `list_hid` with V16's `min_page` floor and an optional string read.
fn list_interfaces(min_page: u16, read_only: bool, with_strings: bool) -> Vec<Interface> {
    let mut found = Vec::new();
    for path in enum_paths() {
        let Some(handle) = open_interface(&path, read_only) else {
            continue;
        };
        let Some(caps) = read_caps(handle.raw()) else {
            continue;
        };
        if caps.UsagePage < min_page {
            continue;
        }
        let (product, manufacturer) = if with_strings {
            hid_strings(handle.raw())
        } else {
            (String::new(), String::new())
        };
        let (vid, pid, is_bluetooth) = parse_ids(&path);
        found.push(Interface {
            dev: HidDevice {
                instance: instance_of(&path),
                path,
                vid,
                pid,
                usage_page: caps.UsagePage,
                usage: caps.Usage,
                product,
                manufacturer,
                is_bluetooth,
            },
            in_len: caps.InputReportByteLength,
            out_len: caps.OutputReportByteLength,
            feat_len: caps.FeatureReportByteLength,
        });
    }
    found
}

// ---------------------------------------------------------------------------
// Overlapped write / read (V16 `_io`, `query_ex`, `read_feature`)
// ---------------------------------------------------------------------------

enum IoOp<'a> {
    Write(&'a [u8]),
    Read(&'a mut [u8]),
}

struct IoOutcome {
    transferred: Option<u32>,
    /// Cancellation could not be confirmed; the caller must leak its buffer.
    leaked: bool,
}

/// One overlapped `WriteFile` / `ReadFile` bounded by `timeout_ms`.
///
/// On timeout the request is cancelled and awaited. If the driver still has not
/// completed the cancellation after `CANCEL_WAIT_MS`, this deliberately leaks the
/// `OVERLAPPED` and reports `leaked` so the caller can leak the buffer too. A
/// one-time leak is the only safe option: freeing the buffer while the kernel may
/// still write into it would be a use-after-free.
fn io_once(handle: HANDLE, op: IoOp<'_>, timeout_ms: u32) -> IoOutcome {
    // SAFETY: NULL attributes / name create a fresh unnamed manual-reset event.
    let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
    if event.is_invalid() {
        return IoOutcome {
            transferred: None,
            leaked: false,
        };
    }

    let mut overlapped = Box::new(OVERLAPPED {
        hEvent: event,
        ..Default::default()
    });
    let mut transferred = 0u32;

    // SAFETY: `handle` is open with FILE_FLAG_OVERLAPPED, the buffers outlive the
    // call (and the wait below) and `overlapped` is a valid, zeroed structure with a
    // private event.
    let started = match op {
        IoOp::Write(buffer) => unsafe {
            WriteFile(
                handle,
                Some(buffer),
                Some(&mut transferred),
                Some(&mut *overlapped),
            )
        },
        IoOp::Read(buffer) => unsafe {
            ReadFile(
                handle,
                Some(buffer),
                Some(&mut transferred),
                Some(&mut *overlapped),
            )
        },
    };

    let mut outcome = IoOutcome {
        transferred: None,
        leaked: false,
    };
    match started {
        Ok(()) => outcome.transferred = Some(transferred),
        Err(error) if error.code().0 == HRESULT_ERROR_IO_PENDING => {
            // SAFETY: waiting on the private event of this overlapped operation.
            if unsafe { WaitForSingleObject(event, timeout_ms) } != WAIT_OBJECT_0 {
                // SAFETY: cancelling only this request; the pointer stays valid.
                let _ = unsafe { CancelIoEx(handle, Some(&*overlapped)) };
                // SAFETY: same private event, bounded wait for the cancellation.
                if unsafe { WaitForSingleObject(event, CANCEL_WAIT_MS) } != WAIT_OBJECT_0 {
                    eprintln!(
                        "[hid] cancellation not confirmed; leaking overlapped/buffer to avoid UAF"
                    );
                    std::mem::forget(overlapped);
                    outcome.leaked = true;
                    return outcome;
                }
                // SAFETY: reaps the (aborted) result so the kernel is done with the
                // buffer; the error code is intentionally ignored (aborted I/O).
                let _ =
                    unsafe { GetOverlappedResult(handle, &*overlapped, &mut transferred, false) };
            } else {
                // SAFETY: the event signalled completion, so this only collects the
                // transferred byte count.
                let _ =
                    unsafe { GetOverlappedResult(handle, &*overlapped, &mut transferred, false) };
                outcome.transferred = Some(transferred);
            }
        }
        Err(_) => {}
    }

    // SAFETY: the event handle is owned by this function and closed exactly once.
    let _ = unsafe { CloseHandle(event) };
    outcome
}

/// V16 `query_ex`: write `out_len` bytes (zero padded) then read `in_len` bytes.
fn query(
    path: &str,
    payload: &[u8],
    out_len: usize,
    in_len: usize,
    timeout_ms: u32,
) -> Option<Vec<u8>> {
    query_matching(path, payload, out_len, in_len, timeout_ms, None)
}

fn query_matching(
    path: &str,
    payload: &[u8],
    out_len: usize,
    in_len: usize,
    timeout_ms: u32,
    profile: Option<&Profile>,
) -> Option<Vec<u8>> {
    let handle = open_path(path, true, true)?;
    let write_timeout = if profile.is_some_and(|p| p.id == "atk-a87-pro") {
        400
    } else {
        timeout_ms
    };

    let mut write_buffer = vec![0u8; out_len.max(1)];
    let copied = payload.len().min(write_buffer.len());
    write_buffer[..copied].copy_from_slice(&payload[..copied]);
    let write = io_once(handle.raw(), IoOp::Write(&write_buffer), write_timeout);
    if write.leaked {
        std::mem::forget(write_buffer);
        return None;
    }
    let written = write.transferred? as usize;
    if profile.is_some() && written != write_buffer.len() {
        return None;
    }

    let deadline = std::time::Instant::now() + Duration::from_millis(u64::from(timeout_ms));
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let mut read_buffer = vec![0u8; in_len.max(1)];
        let read = io_once(
            handle.raw(),
            IoOp::Read(&mut read_buffer),
            remaining.as_millis().clamp(1, u128::from(u32::MAX)) as u32,
        );
        if read.leaked {
            std::mem::forget(read_buffer);
            return None;
        }
        let count = read.transferred? as usize;
        // Preserve legacy zero-byte behavior only for existing unfiltered queries.
        if count == 0 && profile.is_some() {
            return None;
        }
        let count = if count == 0 {
            read_buffer.len()
        } else {
            count.min(read_buffer.len())
        };
        read_buffer.truncate(count);
        if profile.is_none_or(|p| parse_report(p, &read_buffer, false).0.is_some()) {
            return Some(read_buffer);
        }
    }
}

/// V16 `read_feature`: `HidD_GetFeature` on a read-only handle.
fn read_feature(path: &str, report_id: u8, length: usize) -> Option<Vec<u8>> {
    let handle = open_path(path, false, false)?;
    let size = length.clamp(1, 512);
    let mut buffer = vec![0u8; size];
    buffer[0] = report_id;
    // SAFETY: `buffer` is `size` bytes long and the length passed matches it; the
    // handle comes from a successful read-only open.
    let ok = unsafe { HidD_GetFeature(handle.raw(), buffer.as_mut_ptr().cast(), size as u32) };
    ok.then_some(buffer)
}

/// `feature_write_read` 的写半程：`HidD_SetFeature` 把查询帧写进 feature 报告。
fn set_feature(path: &str, payload: &[u8], length: usize) -> bool {
    let Some(handle) = open_path(path, true, false) else {
        return false;
    };
    let size = length.clamp(1, 512);
    let mut buffer = vec![0u8; size];
    let n = payload.len().min(size);
    buffer[..n].copy_from_slice(&payload[..n]);
    // SAFETY: `buffer` is `size` bytes and matches the length passed; the handle
    // comes from a successful read-write open.
    unsafe { HidD_SetFeature(handle.raw(), buffer.as_ptr().cast(), size as u32) }
}

// ---------------------------------------------------------------------------
// Pure helpers (profile driven parsing)
// ---------------------------------------------------------------------------

/// `Some(value)` when non-zero, otherwise `None`. Mirrors Python's `a or b`.
fn nonzero(value: Option<usize>) -> Option<usize> {
    value.filter(|value| *value > 0)
}

/// First of `preferred` (when non-zero) and `fallback` — Python `a or b`.
fn or_len(preferred: Option<usize>, fallback: usize) -> usize {
    nonzero(preferred).unwrap_or(fallback)
}

/// V16 `LINK_BT`: 蓝牙 → Bluetooth only, 2.4G/USB/有线 → non-Bluetooth only,
/// anything else (including empty) → no filter.
fn link_filter(link: &str) -> Option<bool> {
    match link.trim() {
        "蓝牙" => Some(true),
        "2.4G" | "USB" | "有线" => Some(false),
        _ => None,
    }
}

/// Interface filter used by `read_by_profile`.
fn candidate_ok(
    device: &HidDevice,
    vid: &str,
    usage_page: u16,
    usage: Option<u16>,
    want_bluetooth: Option<bool>,
) -> bool {
    if device.usage_page != usage_page || device.vid != vid {
        return false;
    }
    if usage.is_some_and(|usage| device.usage != usage) {
        return false;
    }
    if want_bluetooth.is_some_and(|want| device.is_bluetooth != want) {
        return false;
    }
    true
}

/// `report_id` replaces the first payload byte when both are present.
fn payload_with_report_id(payload: &[u8], report_id: Option<u8>) -> Vec<u8> {
    let mut out = payload.to_vec();
    if let (Some(id), Some(first)) = (report_id, out.first_mut()) {
        *first = id;
    }
    out
}

/// V16 `_parse_level`: validate the raw value, scale it, clamp to 0..=100.
/// `mask` 存在时先按位与（如 `0x7F` 剥掉同字节里的充电位）。
// 参数与 V16 解析规则一一对应；拆结构体会让调用点更难对照原实现。
#[allow(clippy::too_many_arguments)]
fn parse_level(
    raw: &[u8],
    offset: usize,
    u16le: bool,
    min: i64,
    max: i64,
    scale: f64,
    add: f64,
    mask: Option<u8>,
) -> Option<i64> {
    let value = if u16le {
        let low = i64::from(*raw.get(offset)?);
        let high = i64::from(*raw.get(offset + 1)?);
        low | (high << 8)
    } else {
        i64::from(*raw.get(offset)?)
    };
    let value = match mask {
        Some(mask) => value & i64::from(mask),
        None => value,
    };
    if value < min || value > max {
        return None;
    }
    // Python `int(round(x))` uses round-half-to-even; keep the same tie behaviour.
    let scaled = (value as f64) * scale + add;
    Some(scaled.round_ties_even().clamp(0.0, 100.0) as i64)
}

/// V16 `_is_charging`. `mask` 存在时先按位与（如 `0x80` 只看充电位）。
fn is_charging(
    raw: &[u8],
    offset: Option<usize>,
    nonzero: bool,
    when_charging: Option<i64>,
    mask: Option<u8>,
    wired: bool,
) -> bool {
    let Some(offset) = offset else {
        return wired;
    };
    let Some(byte) = raw.get(offset) else {
        return wired;
    };
    let value = i64::from(*byte & mask.unwrap_or(0xFF));
    if let Some(when) = when_charging {
        return wired || value == when;
    }
    if nonzero {
        return wired || value != 0;
    }
    wired
}

/// V16 `_pick_level`. Ties in `ModeNonzero` break to the smallest value.
fn pick_level(values: &[i64], pick: Pick) -> i64 {
    if values.is_empty() {
        return 0;
    }
    match pick {
        Pick::Last => values[values.len() - 1],
        Pick::Median => {
            let mut sorted = values.to_vec();
            sorted.sort_unstable();
            sorted[sorted.len() / 2]
        }
        Pick::ModeNonzero => {
            let mut best = values[0];
            let mut best_count = 0usize;
            for candidate in values {
                let count = values.iter().filter(|value| *value == candidate).count();
                if count > best_count || (count == best_count && *candidate < best) {
                    best = *candidate;
                    best_count = count;
                }
            }
            best
        }
    }
}

/// Turn the collected samples of one interface into a result.
///
/// `None` means "no sample at all": the caller moves on to the next interface.
fn summarize(samples: &[i64], wired: bool, pick: Pick) -> Option<ReadResult> {
    if samples.is_empty() {
        return None;
    }
    let nonzero: Vec<i64> = samples.iter().copied().filter(|value| *value > 0).collect();
    if nonzero.is_empty() {
        // Every reply was a valid 0%: V16 reports sleep, not 0 %.
        return Some(ReadResult {
            state: BatteryState::Sleep,
            level: None,
            charging: wired,
        });
    }
    Some(ReadResult {
        state: BatteryState::Ok,
        level: Some(pick_level(&nonzero, pick).clamp(0, 100) as u8),
        charging: wired,
    })
}

// ---------------------------------------------------------------------------
// Read pipeline
// ---------------------------------------------------------------------------

/// Read one sample from one interface.
fn sample(
    interface: &Interface,
    wired: bool,
    profile: &Profile,
    payload: &[u8],
) -> (Option<i64>, bool) {
    let request = &profile.request;

    let raw = match request.transport {
        Transport::FeatureRead => {
            let report_id = request.report_id.unwrap_or(0);
            let mut length = or_len(
                request.in_len.map(|len| len as usize),
                interface.feat_len as usize,
            );
            if length == 0 {
                length = 64;
            }
            read_feature(&interface.dev.path, report_id, length)
        }
        Transport::FeatureWriteRead => {
            // SetFeature（查询帧）→ 等固件暂存应答 → GetFeature（同 report id）。
            // 凌豹 M5PRO / EWEADN S9 Pro 实测 50~100ms 后应答就绪。
            let report_id = request
                .report_id
                .or_else(|| payload.first().copied())
                .unwrap_or(0);
            let mut out_len = or_len(
                request.out_len.map(|len| len as usize),
                interface.feat_len as usize,
            );
            if out_len == 0 {
                out_len = 64;
            }
            if !set_feature(&interface.dev.path, payload, out_len) {
                return (None, wired);
            }
            std::thread::sleep(std::time::Duration::from_millis(80));
            let mut in_len = or_len(
                request.in_len.map(|len| len as usize),
                interface.feat_len as usize,
            );
            if in_len == 0 {
                in_len = 64;
            }
            read_feature(&interface.dev.path, report_id, in_len)
        }
        Transport::WriteRead => {
            let out_len = or_len(
                request.out_len.map(|len| len as usize),
                interface.out_len as usize,
            );
            let in_len = or_len(
                request.in_len.map(|len| len as usize),
                interface.in_len as usize,
            );
            let timeout_ms = profile.sampling.request_timeout_ms as u32;
            if profile.id == "atk-a87-pro" && (interface.out_len != 33 || interface.in_len != 33) {
                return (None, wired);
            }
            query_matching(
                &interface.dev.path,
                payload,
                out_len,
                in_len,
                timeout_ms,
                request.read_matching.then_some(profile),
            )
        }
    };

    let Some(raw) = raw else {
        return (None, wired);
    };
    debug_log(&format!(
        "raw {} {} {}",
        interface.dev.vid,
        interface.dev.pid,
        raw.iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    parse_report(profile, &raw, wired)
}

fn parse_report(profile: &Profile, raw: &[u8], wired: bool) -> (Option<i64>, bool) {
    let response = &profile.response;
    if raw.len() < response.min_length
        || response
            .exact_length
            .is_some_and(|length| raw.len() != length)
        || response
            .markers
            .iter()
            .any(|(offset, byte)| raw.get(usize::from(*offset)) != Some(byte))
        || (profile.id == "atk-a87-pro" && raw.get(4) == Some(&0) && raw.get(5) == Some(&0))
    {
        return (None, wired);
    }
    if profile.id == "hyperx-cloud-alpha-wireless" && !raw.starts_with(&[0x21, 0xBB, 0x0B]) {
        return (None, wired);
    }
    if let Some(offset) = response.status.offset.map(|offset| offset as usize) {
        let Some(byte) = raw.get(offset) else {
            return (None, wired);
        };
        // A status byte outside ok_values invalidates this sample.
        if !response
            .status
            .ok_values
            .iter()
            .any(|value| *value as i64 == i64::from(*byte))
        {
            return (None, wired);
        }
    }

    let level_rule = &response.level;
    let level = parse_level(
        raw,
        level_rule.offset as usize,
        matches!(level_rule.encoding, LevelEncoding::U16Le),
        level_rule.min as i64,
        level_rule.max as i64,
        level_rule.scale,
        level_rule.add,
        level_rule.mask,
    );
    let Some(level) = level else {
        return (None, wired);
    };

    let charging_rule = &response.charging;
    let charging = is_charging(
        raw,
        charging_rule.offset.map(|offset| offset as usize),
        charging_rule.nonzero,
        charging_rule.when_charging.map(|value| value as i64),
        charging_rule.mask,
        wired,
    );
    (Some(level), charging)
}

/// Sample without feeding a previous report's charging bit back into the wired-PID hint.
fn collect_samples(
    attempts: usize,
    stable: usize,
    wired_by_pid: bool,
    mut read: impl FnMut(bool) -> (Option<i64>, bool),
) -> (Vec<i64>, bool) {
    let mut samples = Vec::with_capacity(attempts);
    let mut charging = wired_by_pid;
    for _ in 0..attempts {
        let (level, current_charging) = read(wired_by_pid);
        let Some(level) = level else {
            continue;
        };
        charging = current_charging;
        samples.push(level);
        if samples.len() >= stable {
            let tail = &samples[samples.len() - stable..];
            if tail.iter().all(|value| *value == tail[0]) {
                break;
            }
        }
    }
    (samples, charging)
}

fn matches_instance(device: &HidDevice, instance: &str) -> bool {
    instance.is_empty() || device.instance == instance
}

fn matches_profile_pid(device: &HidDevice, profile: &Profile) -> bool {
    // New filtered protocols never widen a read to another product of the vendor.
    !profile.request.read_matching
        || profile.known_ids.iter().any(|id| {
            device.vid.eq_ignore_ascii_case(&id.vid)
                && (device.pid.eq_ignore_ascii_case(&id.pid)
                    || (!id.wired_pid.is_empty() && device.pid.eq_ignore_ascii_case(&id.wired_pid)))
        })
}

/// Try every candidate interface in order; the first one that answers wins.
///
/// `None` means the candidate list was empty (V16 `found` stays false).
fn try_interfaces(
    candidates: &[&Interface],
    profile: &Profile,
    payload: &[u8],
    wired_pid: &str,
) -> Option<ReadResult> {
    if candidates.is_empty() {
        return None;
    }
    let attempts = (profile.sampling.attempts as usize).max(1);
    let stable = (profile.sampling.stable_count as usize).max(1);
    let pick = profile.sampling.pick;

    for interface in candidates {
        // An interface whose PID is the wired PID is charging by definition.
        let wired_by_pid = !wired_pid.is_empty() && interface.dev.pid == wired_pid;
        let (samples, charging) = collect_samples(attempts, stable, wired_by_pid, |wired| {
            sample(interface, wired, profile, payload)
        });
        let result = if profile.request.read_matching && !samples.is_empty() {
            // These new protocols validate sleep before sampling; a valid 0 is 0%.
            Some(ReadResult {
                state: BatteryState::Ok,
                level: Some(pick_level(&samples, pick).clamp(0, 100) as u8),
                charging,
            })
        } else {
            summarize(&samples, charging, pick)
        };
        if let Some(result) = result {
            debug_log(&format!(
                "iface answer {} samples {:?} -> {}",
                interface.dev.path,
                samples,
                result.level.unwrap_or(255)
            ));
            return Some(result);
        }
        debug_log(&format!(
            "iface no-answer {} samples {:?}",
            interface.dev.path, samples
        ));
        // No usable sample (write rejected / no reply): try the next interface.
    }

    // Interfaces matched but none answered: the device is asleep, not absent.
    Some(ReadResult {
        state: BatteryState::Sleep,
        level: None,
        charging: false,
    })
}

/// The actual read; runs on the throw-away thread owned by [`read_by_profile`].
fn read_once(dev: &DeviceSpec, profile: &Profile) -> ReadResult {
    let rule = &profile.match_rule;
    let usage_page = rule.usage_page;
    let usage = rule.usage;
    let vid = dev.vid.to_ascii_uppercase();
    let pid = dev.pid.to_ascii_uppercase();
    let wired_pid = dev.wired_pid.to_ascii_uppercase();
    let want_bluetooth = link_filter(&dev.link);
    let payload = payload_with_report_id(&profile.request.payload, profile.request.report_id);

    // Vendor-page profiles only need the vendor range; others must see every page.
    let floor = if usage_page >= 0xFF00 { 0xFF00 } else { 0 };
    let matching: Vec<Interface> = list_interfaces(floor, true, false)
        .into_iter()
        .filter(|interface| candidate_ok(&interface.dev, &vid, usage_page, usage, want_bluetooth))
        .collect();
    // 已绑定的接收器必须严格匹配；拔出或换端口后由添加页重新绑定，
    // 不得静默读取另一台同 VID/PID 接收器。旧配置未存实例时保留兼容。
    let all: Vec<Interface> = matching
        .into_iter()
        .filter(|interface| matches_instance(&interface.dev, &dev.instance))
        .filter(|interface| matches_profile_pid(&interface.dev, profile))
        .collect();

    // `require_pid_hit` is inverted: `false` means "exact PID first, then widen".
    if (!pid.is_empty() || !wired_pid.is_empty()) && !rule.require_pid_hit {
        let exact: Vec<&Interface> = all
            .iter()
            .filter(|interface| {
                interface.dev.pid == pid
                    || (!wired_pid.is_empty() && interface.dev.pid == wired_pid)
            })
            .collect();
        for interface in &exact {
            debug_log(&format!(
                "read {}:{} iface usage {:04X}:{:04X} out {} in {} path {}",
                vid,
                pid,
                interface.dev.usage_page,
                interface.dev.usage,
                interface.out_len,
                interface.in_len,
                interface.dev.path
            ));
        }
        if let Some(result) = try_interfaces(&exact, profile, &payload, &wired_pid) {
            return result;
        }
    }
    let wide: Vec<&Interface> = all.iter().collect();
    try_interfaces(&wide, profile, &payload, &wired_pid).unwrap_or(ReadResult {
        state: BatteryState::Absent,
        level: None,
        charging: false,
    })
}

/// Read one device with the given profile.
///
/// The read runs on a detached thread; `sampling.total_timeout_s` bounds the wait
/// (V16 `_read_with_timeout`). A device that never answers yields [`BatteryState::Sleep`].
pub fn read_by_profile(dev: &DeviceSpec, profile: &Profile) -> ReadResult {
    let timeout = sanitized_timeout(profile.sampling.total_timeout_s);
    let owned_dev = dev.clone();
    let owned_profile = profile.clone();
    let (sender, receiver) = mpsc::channel();

    let spawned = thread::Builder::new()
        .name("battery-read".to_string())
        .spawn(move || {
            let _ = sender.send(read_once(&owned_dev, &owned_profile));
        });

    let result = match spawned {
        Err(_) => ReadResult {
            state: BatteryState::Error,
            level: None,
            charging: false,
        },
        Ok(_detached) => match receiver.recv_timeout(Duration::from_secs_f64(timeout)) {
            Ok(result) => result,
            // Still running after the budget: treat as sleep (V16 `t.is_alive()`).
            Err(RecvTimeoutError::Timeout) => ReadResult {
                state: BatteryState::Sleep,
                level: None,
                charging: false,
            },
            // The worker died (panic): V16 mapped the exception to `error`.
            Err(RecvTimeoutError::Disconnected) => ReadResult {
                state: BatteryState::Error,
                level: None,
                charging: false,
            },
        },
    };

    log_read(dev, profile, &result);
    result
}

fn sanitized_timeout(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 && value <= 3600.0 {
        value
    } else {
        DEFAULT_TOTAL_TIMEOUT_S
    }
}

// ---------------------------------------------------------------------------
// Compx session ACK (model code disambiguation)
// ---------------------------------------------------------------------------

/// 从 Compx 会话 ACK 里取型号码（大端第 9/10 字节）。
///
/// 会话帧 `13 05 01 …`；ACK 形如 `13 05 01 00 0A 03 00 00 00 02 12 01 … 3B`。
/// `3554:FA09` 这类公版接收器被多个品牌共用，HID 层与产品名都分不出来，
/// 只能靠这里的型号码分流（K98=0x0212、AULA F99 Pro=0x0158；依据 vigil-watt
/// `CompxSessionAck`）。Windows 的读取缓冲有时带 report ID 前缀，0/1 两个偏移都试。
pub fn parse_compx_model(raw: &[u8]) -> Option<u16> {
    for offset in [0usize, 1] {
        let end = offset + 12;
        if raw.len() < end {
            continue;
        }
        if raw[offset] == 0x13 && raw[offset + 1] == 0x05 && raw[offset + 2] == 0x01 {
            return Some(u16::from_be_bytes([raw[offset + 9], raw[offset + 10]]));
        }
    }
    None
}

/// 给 `vid:pid` 的 20/20 `0xFF02/0x0002` 接口发一次会话帧，读回型号码。
///
/// 只读场景也要写这一帧（设备本来就要先开会话才应答），调用方应只在
/// 同 VID/PID 存在多个品牌协议时使用。设备休眠 / 会话被占用时返回 `None`，
/// 由调用方退回不冒用品牌名的中性显示。
///
/// 单次等待 350ms：扫描列表只需要品牌提示，睡眠设备不值得为它把添加窗拖住；
/// 读不到就用中性名，用户仍可在协议下拉里手动选。
/// 按接收器实例读型号码：两个同 VID/PID 接收器必须各读各的。
pub fn read_model_code_for(vid: &str, pid: &str, instance: &str) -> Option<u16> {
    let mut payload = [0u8; 20];
    payload[0] = 0x13;
    payload[1] = 0x05;
    payload[2] = 0x01;
    payload[19] = 0x13u8.wrapping_add(0x05).wrapping_add(0x01);
    for interface in list_interfaces(0, true, false) {
        let device = &interface.dev;
        if device.vid != vid || device.pid != pid {
            continue;
        }
        if !instance.is_empty() && device.instance != instance {
            continue;
        }
        if device.usage_page != 0xFF02 || device.usage != 0x0002 {
            continue;
        }
        if interface.out_len as usize != payload.len()
            || (interface.in_len as usize) < payload.len()
        {
            continue;
        }
        // 实测第一帧就是 ACK；部分固件会先推一条 0A 电量帧，再试一次即可。
        for _ in 0..2 {
            if let Some(raw) = query(
                &interface.dev.path,
                &payload,
                payload.len(),
                payload.len(),
                350,
            ) && let Some(code) = parse_compx_model(&raw)
            {
                return Some(code);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Debug logging (V16 battery.py `[read] ...`)
// ---------------------------------------------------------------------------

fn debug_enabled() -> bool {
    let is_one = |name: &str| {
        std::env::var(name)
            .map(|value| value == "1")
            .unwrap_or(false)
    };
    is_one("EZB_DEBUG") || is_one("BP_DEBUG")
}

/// 调试日志：exe 同级 `battery.log`，跟随便携目录。
fn debug_log(line: &str) {
    if !debug_enabled() {
        return;
    }
    use std::io::Write as _;
    let dir = crate::config::program_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("battery.log"))
    {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis())
            .unwrap_or(0);
        let _ = writeln!(file, "{stamp} {line}");
    }
}

fn state_text(state: BatteryState) -> &'static str {
    match state {
        BatteryState::Ok => "ok",
        BatteryState::Sleep => "sleep",
        BatteryState::Absent => "absent",
        BatteryState::Error => "error",
    }
}

/// `[read] <id> <label> -> <state> <level> (<profile>)`, same field order as V16.
fn log_read(dev: &DeviceSpec, profile: &Profile, result: &ReadResult) {
    if !debug_enabled() {
        return;
    }
    let level = match result.level {
        Some(level) => level.to_string(),
        None => "None".to_string(),
    };
    println!(
        "[read] {} {} -> {} {} ({})",
        dev.id,
        dev.label,
        state_text(result.state),
        level,
        profile.id
    );
    debug_log(&format!(
        "read {} {} -> {} {} ({})",
        dev.id,
        dev.label,
        state_text(result.state),
        level,
        profile.id
    ));
    use std::io::Write as _;
    let _ = std::io::stdout().flush();
}
