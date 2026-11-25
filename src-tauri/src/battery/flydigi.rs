//! Flydigi Space Station: local, read-only device-list query for APEX 4 (2.4G).
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING,
    ReadFile, WriteFile,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::{PCWSTR, w};

use super::{BatteryState, DeviceSpec, ReadResult};

pub const ID: &str = "flydigi-apex4";
pub const PROFILE: &str = "flydigi-space-station";
pub const LABEL: &str = "飞智 Flydigi 八爪鱼 4";
pub const PROFILE_LABEL: &str = "飞智空间站 · 2.4G";
pub const VID: &str = "FLYDIGI";
pub const PID: &str = "K2";

pub fn matches(device: &DeviceSpec) -> bool {
    device.vid.eq_ignore_ascii_case(VID) && device.pid.eq_ignore_ascii_case(PID)
}

// Framing reference: AweStudioX/DualSenseBatteryTray, FlydigiServiceClient.cs.
const HEADER: &[u8; 13] = b"FDG_PROTOCOL\n";
const REQUEST: &[u8] = b"FDG_PROTOCOL\n\x05\x00\x00\x00\x08\x81\x20\x1a\x00";
const MAX_PAYLOAD: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_millis(1500);
const IO_PENDING: i32 = 0x8007_03e5u32 as i32;
// SECURITY_SQOS_PRESENT | SECURITY_ANONYMOUS: the server cannot impersonate us.
const ANONYMOUS_SQOS: u32 = 0x0010_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Unavailable,
    Offline,
    Online(Option<u64>),
}

impl Status {
    pub fn reading(self) -> ReadResult {
        match self {
            Self::Online(Some(level @ 0..=5)) => ReadResult {
                state: BatteryState::Ok,
                level: Some([5, 20, 40, 60, 80, 100][level as usize]),
                charging: false,
            },
            Self::Online(level) => ReadResult {
                state: BatteryState::Ok,
                level: None,
                charging: level == Some(6),
            },
            _ => ReadResult {
                state: BatteryState::Absent,
                ..ReadResult::default()
            },
        }
    }
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], ()> {
    let (head, tail) = bytes.split_at_checked(len).ok_or(())?;
    *bytes = tail;
    Ok(head)
}

fn varint(bytes: &mut &[u8]) -> Result<u64, ()> {
    let mut value = 0;
    for shift in (0..64).step_by(7) {
        let byte = take(bytes, 1)?[0];
        if shift == 63 && byte > 1 {
            return Err(());
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte < 128 {
            return Ok(value);
        }
    }
    Err(())
}

enum Field<'a> {
    Integer(u64),
    Bytes(&'a [u8]),
    Ignored,
}

fn next<'a>(bytes: &mut &'a [u8]) -> Result<(u64, Field<'a>), ()> {
    let tag = varint(bytes)?;
    let number = tag >> 3;
    if !(1..=(1 << 29) - 1).contains(&number) {
        return Err(());
    }
    let field = match tag & 7 {
        0 => Field::Integer(varint(bytes)?),
        2 => {
            let len = usize::try_from(varint(bytes)?).map_err(|_| ())?;
            Field::Bytes(take(bytes, len)?)
        }
        1 | 5 => {
            take(bytes, if tag & 7 == 1 { 8 } else { 4 })?;
            Field::Ignored
        }
        _ => return Err(()),
    };
    Ok((number, field))
}

fn device(mut bytes: &[u8]) -> Result<Option<Option<u64>>, ()> {
    let (mut code, mut link, mut connected, mut level) = (&b""[..], 0, false, None);
    while !bytes.is_empty() {
        match next(&mut bytes)? {
            (4, Field::Bytes(value)) => code = value,
            (3, Field::Integer(value)) => link = value,
            (8, Field::Integer(value)) => connected = value == 1,
            (9, Field::Integer(value)) => level = Some(value),
            _ => {}
        }
    }
    Ok((code == b"k2" && link == 2 && connected).then_some(level))
}

fn response(mut bytes: &[u8]) -> Result<Option<Status>, ()> {
    if bytes.len() > MAX_PAYLOAD {
        return Err(());
    }
    let (mut cmd, mut found, mut ambiguous) = (0, None, false);
    while !bytes.is_empty() {
        match next(&mut bytes)? {
            (1, Field::Integer(value)) => cmd = value,
            (5 | 6, Field::Bytes(mut list)) => {
                while !list.is_empty() {
                    if let (1, Field::Bytes(row)) = next(&mut list)?
                        && let Some(level) = device(row)?
                    {
                        ambiguous |= found.replace(level).is_some();
                    }
                }
            }
            _ => {}
        }
    }
    Ok((cmd == 4097).then(|| {
        if ambiguous {
            Status::Unavailable
        } else {
            found.map_or(Status::Offline, Status::Online)
        }
    }))
}

struct Operation {
    event: OwnedHandle,
    overlapped: Box<OVERLAPPED>,
    buffer: Vec<u8>,
}

// SAFETY: the only pointer-like field used in OVERLAPPED is the owned event
// handle. The boxed OVERLAPPED and Vec allocation never move while I/O is pending.
// Only the mutex owner accesses them, and pending operations are not touched until signalled.
unsafe impl Send for Operation {}

struct Pending {
    _pipe: OwnedHandle,
    operation: Operation,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn raw(handle: &OwnedHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}

fn io(
    pipe: &OwnedHandle,
    buffer: Vec<u8>,
    write: bool,
    deadline: Instant,
) -> Result<(Vec<u8>, usize), Option<Operation>> {
    if Instant::now() >= deadline {
        return Err(None);
    }
    // SAFETY: unnamed private event, and each successful handle is owned once.
    let event = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }.map_err(|_| None)?;
    let mut op = Operation {
        event: unsafe { OwnedHandle::from_raw_handle(event.0) },
        overlapped: Box::new(OVERLAPPED { hEvent: event, ..Default::default() }),
        buffer,
    };
    // SAFETY: both kernel-accessed allocations stay alive until completion, or are
    // returned to the caller's pending slot on timeout. No stack buffer is passed.
    let overlapped = std::ptr::from_mut(op.overlapped.as_mut());
    let start = unsafe {
        if write {
            WriteFile(raw(pipe), Some(&op.buffer), None, Some(overlapped))
        } else {
            ReadFile(raw(pipe), Some(&mut op.buffer), None, Some(overlapped))
        }
    };
    if let Err(error) = start {
        if error.code().0 != IO_PENDING {
            return Err(None);
        }
        let wait = deadline.saturating_duration_since(Instant::now());
        if unsafe { WaitForSingleObject(event, wait.as_millis() as u32) } != WAIT_OBJECT_0 {
            let _ = unsafe { CancelIoEx(raw(pipe), Some(overlapped.cast_const())) };
            return Err(Some(op));
        }
    }
    let mut count = 0;
    unsafe { GetOverlappedResult(raw(pipe), overlapped.cast_const(), &mut count, false) }
        .map_err(|_| None)?;
    Ok((op.buffer, count as usize))
}

struct Query<'a> {
    pipe: Option<OwnedHandle>,
    pending: &'a mut Option<Pending>,
    deadline: Instant,
}

impl Query<'_> {
    fn transfer(&mut self, buffer: Vec<u8>, write: bool) -> Result<(Vec<u8>, usize), ()> {
        match io(self.pipe.as_ref().ok_or(())?, buffer, write, self.deadline) {
            Ok(result) => Ok(result),
            Err(operation) => {
                if let Some(operation) = operation {
                    *self.pending = Some(Pending { _pipe: self.pipe.take().ok_or(())?, operation });
                }
                Err(())
            }
        }
    }

    fn read_exact(&mut self, size: usize) -> Result<Vec<u8>, ()> {
        let mut result = Vec::with_capacity(size);
        while result.len() < size {
            let remaining = size - result.len();
            let (buffer, count) = self.transfer(vec![0; remaining], false)?;
            if count == 0 || count > remaining {
                return Err(());
            }
            result.extend_from_slice(&buffer[..count]);
        }
        Ok(result)
    }

    fn run(&mut self) -> Result<Status, ()> {
        let (_, count) = self.transfer(REQUEST.to_vec(), true)?;
        if count != REQUEST.len() {
            return Err(());
        }
        for _ in 0..4 {
            let header = self.read_exact(HEADER.len() + 4)?;
            if !header.starts_with(HEADER) {
                return Err(());
            }
            let len = u32::from_le_bytes(header[HEADER.len()..].try_into().map_err(|_| ())?) as usize;
            if len > MAX_PAYLOAD {
                return Err(());
            }
            if let Some(result) = response(&self.read_exact(len)?)? {
                return Ok(result);
            }
        }
        Err(())
    }
}

pub fn query() -> Status {
    let deadline = Instant::now() + TIMEOUT;
    let Ok(mut pending) = PENDING.try_lock() else {
        return Status::Unavailable;
    };
    if let Some(existing) = pending.as_ref()
        && unsafe { WaitForSingleObject(raw(&existing.operation.event), 0) } != WAIT_OBJECT_0
    {
        return Status::Unavailable;
    }
    // The event signals completion (including cancellation); it is now safe to free the buffer.
    *pending = None;
    let Ok(pipe) = (unsafe {
        CreateFileW(
            w!(r"\\.\pipe\fcs.sock"),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(FILE_FLAG_OVERLAPPED.0 | ANONYMOUS_SQOS),
            None,
        )
    }) else {
        return Status::Unavailable;
    };
    let pipe = unsafe { OwnedHandle::from_raw_handle(pipe.0) };
    Query { pipe: Some(pipe), pending: &mut pending, deadline }
        .run()
        .unwrap_or(Status::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(code: &[u8; 2], link: u8, online: u8, level: u8) -> Vec<u8> {
        vec![8, 0x81, 0x20, 42, 12, 10, 10, 34, 2, code[0], code[1], 24, link, 64, online, 72, level]
    }

    #[test]
    fn accepts_only_connected_wireless_apex4() {
        assert_eq!(response(&fixture(b"k2", 2, 1, 4)), Ok(Some(Status::Online(Some(4)))));
        for (code, link, online) in [(b"k3", 2, 1), (b"k2", 1, 1), (b"k2", 2, 0)] {
            assert_eq!(response(&fixture(code, link, online, 4)), Ok(Some(Status::Offline)));
        }
        let mut duplicate = fixture(b"k2", 2, 1, 4);
        duplicate.extend_from_slice(&fixture(b"k2", 2, 1, 2)[3..]);
        assert_eq!(response(&duplicate), Ok(Some(Status::Unavailable)));
    }

    #[test]
    fn maps_battery_without_inventing_a_percentage() {
        for (level, percent) in [5, 20, 40, 60, 80, 100].into_iter().enumerate() {
            let actual = Status::Online(Some(level as u64)).reading();
            assert_eq!(actual.level, Some(percent));
            assert!(!actual.charging);
        }
        let charging = Status::Online(Some(6)).reading();
        assert!(charging.charging);
        assert_eq!(charging.level, None);
        assert_eq!(Status::Online(Some(7)).reading().level, None);
        assert_eq!(Status::Unavailable.reading().level, None);
    }

    #[test]
    fn rejects_invalid_frames_and_ignores_other_commands() {
        let mut frame = fixture(b"k2", 2, 1, 4);
        frame.pop();
        assert_eq!(response(&frame), Err(()));
        assert_eq!(response(&[0x80; 11]), Err(()));
        assert_eq!(response(&vec![0; MAX_PAYLOAD + 1]), Err(()));
        assert_eq!(response(&[8, 1]), Ok(None));
    }
}
