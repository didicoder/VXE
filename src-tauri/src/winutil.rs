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

pub fn taskbar_edge(work: Rect, screen_w: i32, screen_h: i32) -> TaskbarEdge {
    if work.bottom < screen_h - 2 {
        TaskbarEdge::Bottom
    } else if work.top > 2 {
        TaskbarEdge::Top
    } else if work.left > 2 {
        TaskbarEdge::Left
    } else if work.right < screen_w - 2 {
        TaskbarEdge::Right
    } else {
        let cy = (work.top + work.bottom) / 2;
        if cy >= screen_h / 2 {
            TaskbarEdge::Bottom
        } else {
            TaskbarEdge::Top
        }
    }
}

pub fn anchor_for_rect(
    rect: Rect,
    width: i32,
    height: i32,
    gap: i32,
    screen_w: i32,
    screen_h: i32,
    work: Rect,
) -> (i32, i32) {
    let cx = (rect.left + rect.right) / 2;
    let cy = (rect.top + rect.bottom) / 2;
    let (mut x, mut y) = match taskbar_edge(work, screen_w, screen_h) {
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
pub fn work_area_for_point(x: i32, y: i32) -> Option<Rect> {
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
    ok.as_bool().then_some(Rect {
        left: info.rcWork.left,
        top: info.rcWork.top,
        right: info.rcWork.right,
        bottom: info.rcWork.bottom,
    })
}

#[cfg(windows)]
pub fn primary_screen_size() -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    // SAFETY: GetSystemMetrics is a pure query with no pointer arguments.
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}
