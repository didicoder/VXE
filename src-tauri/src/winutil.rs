#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskbarEdge {
    Top,
    Bottom,
    Left,
    Right,
}

pub fn taskbar_edge(monitor: Rect, work: Rect) -> TaskbarEdge {
    if monitor.bottom - work.bottom > 2 {
        TaskbarEdge::Bottom
    } else if work.top - monitor.top > 2 {
        TaskbarEdge::Top
    } else if work.left - monitor.left > 2 {
        TaskbarEdge::Left
    } else if monitor.right - work.right > 2 {
        TaskbarEdge::Right
    } else {
        TaskbarEdge::Bottom
    }
}

pub fn anchor_for_rect(
    rect: Rect,
    width: i32,
    height: i32,
    gap: i32,
    monitor: Rect,
    work: Rect,
) -> (i32, i32) {
    let cx = (rect.left + rect.right) / 2;
    let cy = (rect.top + rect.bottom) / 2;
    let (mut x, mut y) = match taskbar_edge(monitor, work) {
        TaskbarEdge::Bottom => (cx - width / 2, rect.top - height - gap),
        TaskbarEdge::Top => (cx - width / 2, rect.bottom + gap),
        TaskbarEdge::Left => (rect.right + gap, cy - height / 2),
        TaskbarEdge::Right => (rect.left - width - gap, cy - height / 2),
    };
    x = x.max(work.left + 4).min(work.right - width - 4);
    y = y.max(work.top + 4).min(work.bottom - height - 4);
    (x, y)
}

#[cfg(windows)]
pub fn monitor_and_work_area_for_point(x: i32, y: i32) -> Option<(Rect, Rect)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: monitor/point are plain Win32 values; `info` is a valid out-parameter
    // whose cbSize is initialized as required by GetMonitorInfoW.
    let ok = unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(monitor, &mut info)
    };
    ok.as_bool().then_some((
        Rect {
            left: info.rcMonitor.left,
            top: info.rcMonitor.top,
            right: info.rcMonitor.right,
            bottom: info.rcMonitor.bottom,
        },
        Rect {
            left: info.rcWork.left,
            top: info.rcWork.top,
            right: info.rcWork.right,
            bottom: info.rcWork.bottom,
        },
    ))
}

#[cfg(windows)]
pub fn work_area_for_point(x: i32, y: i32) -> Option<Rect> {
    monitor_and_work_area_for_point(x, y).map(|(_, work)| work)
}

#[cfg(windows)]
pub fn primary_screen_size() -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    // SAFETY: GetSystemMetrics is a pure query with no pointer arguments.
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}
