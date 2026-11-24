//! 托盘图标的纯函数栅格化：闪电 / 圆环 / 电池三种样式。
//!
//! 三种样式共用同一条 64×64 RGBA + 4×4 超采样路径，只替换几何谓词；
//! 每帧只分配一张 16 KB 位图，不做样式×电量×状态的全量预生成。
//! 低电量闪烁只预渲染当前样式的两帧（约 32 KB），由 `tray.rs` 交替切换。

pub const ICON_SIZE: u32 = 64;

/// 闪电顶点（24×24 设计坐标，与 HTML 的 path 一致）。
const BOLT: [(f32, f32); 6] = [
    (13.5, 2.5),
    (6.0, 13.5),
    (11.0, 13.5),
    (10.0, 21.5),
    (18.0, 10.5),
    (12.5, 10.5),
];
const TOP: f32 = 2.5;
const BOTTOM: f32 = 21.5;
const STROKE_HALF_WIDTH: f32 = 0.75;
const STROKE_OPACITY: f32 = 0.63;

const RING_CENTER: f32 = 32.0;
const RING_RADIUS: f32 = 24.0;
const RING_HALF_WIDTH: f32 = 4.0;
const RING_TRACK_OPACITY: f32 = 0.32;

/// 电池外壳中心线矩形 (x, y, w, h, r)；描边 4 后外沿 6..52 / 16..48。
const BATTERY_OUTER: (f32, f32, f32, f32, f32) = (6.0, 18.0, 46.0, 28.0, 6.0);
const BATTERY_INNER: (f32, f32, f32, f32, f32) = (10.0, 22.0, 38.0, 20.0, 3.0);
const BATTERY_TERMINAL: (f32, f32, f32, f32, f32) = (54.0, 26.0, 4.0, 12.0, 2.0);
const BATTERY_MIN_FRACTION: f32 = 0.06;

const SUPERSAMPLE: u32 = 4;
const LEVEL_STEP: u8 = 5;

pub const GREEN: [u8; 3] = [0x3c, 0xe2, 0x8a];
pub const YELLOW: [u8; 3] = [0xf0, 0xad, 0x4e];
pub const RED: [u8; 3] = [0xff, 0x4d, 0x4f];
pub const BLUE: [u8; 3] = [0x5b, 0x9c, 0xf5];

/// 托盘图标样式，与设置页 `tray_icon_style` 三档一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconStyle {
    Bolt,
    Ring,
    Battery,
}

impl IconStyle {
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "ring" => Self::Ring,
            "battery" => Self::Battery,
            _ => Self::Bolt,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bolt => "bolt",
            Self::Ring => "ring",
            Self::Battery => "battery",
        }
    }

    fn accent_opacity(self) -> f32 {
        match self {
            Self::Bolt => STROKE_OPACITY,
            Self::Ring => RING_TRACK_OPACITY,
            // 电池外壳/正极全色描边，低电量空壳仍要清晰。
            Self::Battery => 1.0,
        }
    }
}

/// 电量档位，与设置页「电量充足 / 正常使用 / 电量不足」一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    Low,
    Mid,
    Good,
}

impl Tier {
    #[must_use]
    pub fn of(level: u8, good: u8, warn: u8) -> Self {
        if level >= good {
            Self::Good
        } else if level >= warn {
            Self::Mid
        } else {
            Self::Low
        }
    }

    #[must_use]
    pub fn color(self) -> [u8; 3] {
        match self {
            Self::Low => RED,
            Self::Mid => YELLOW,
            Self::Good => GREEN,
        }
    }
}

/// 图标按 5% 一档：同一个 5% 区间渲染结果完全一致。
#[must_use]
pub fn quantize(level: u8) -> u8 {
    level.min(100) / LEVEL_STEP * LEVEL_STEP
}

/// 一个采样点的墨色：Fill 优先于 Accent。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ink {
    None,
    Accent,
    Fill,
}

/// 当前样式的几何参数；每帧只构造一次。
#[derive(Clone, Copy)]
enum Shape {
    Bolt { clip_top: f32 },
    Ring { sweep_deg: f32 },
    Battery { fill_w: f32 },
}

impl Shape {
    fn of(style: IconStyle, level: u8, alert_off: bool) -> Self {
        let level = f32::from(quantize(level));
        match style {
            IconStyle::Bolt => Self::Bolt {
                clip_top: BOTTOM - (BOTTOM - TOP) * level / 100.0,
            },
            IconStyle::Ring => Self::Ring {
                sweep_deg: if alert_off { 0.0 } else { level * 3.6 },
            },
            IconStyle::Battery => Self::Battery {
                fill_w: if alert_off {
                    0.0
                } else {
                    BATTERY_INNER.2 * (level / 100.0).clamp(BATTERY_MIN_FRACTION, 1.0)
                },
            },
        }
    }

    fn unit(self) -> f32 {
        match self {
            Self::Bolt { .. } => 24.0 / ICON_SIZE as f32,
            _ => 1.0,
        }
    }

    fn ink(self, x: f32, y: f32) -> Ink {
        match self {
            Self::Bolt { clip_top } => {
                if inside_polygon(x, y) && y >= clip_top {
                    Ink::Fill
                } else if distance_to_bolt(x, y) <= STROKE_HALF_WIDTH {
                    Ink::Accent
                } else {
                    Ink::None
                }
            }
            Self::Ring { sweep_deg } => {
                let dx = x - RING_CENTER;
                let dy = y - RING_CENTER;
                if (dx.hypot(dy) - RING_RADIUS).abs() > RING_HALF_WIDTH {
                    return Ink::None;
                }
                if sweep_deg >= 360.0 {
                    return Ink::Fill;
                }
                let angle = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
                if angle <= sweep_deg {
                    Ink::Fill
                } else {
                    Ink::Accent
                }
            }
            Self::Battery { fill_w } => {
                let shell = rounded_rect_contains(x, y, BATTERY_OUTER)
                    && !rounded_rect_contains(x, y, BATTERY_INNER);
                let terminal = rounded_rect_contains(x, y, BATTERY_TERMINAL);
                let fill = fill_w > 0.0
                    && rounded_rect_contains(
                        x,
                        y,
                        (
                            BATTERY_INNER.0,
                            BATTERY_INNER.1,
                            fill_w,
                            BATTERY_INNER.3,
                            BATTERY_INNER.4,
                        ),
                    );
                if shell || terminal || fill {
                    Ink::Fill
                } else {
                    Ink::None
                }
            }
        }
    }
}

/// 渲染指定样式与电量；`color` 决定档位色，填充比例由 5% 量化后的电量决定。
#[must_use]
pub fn render_style(style: IconStyle, level: u8, color: [u8; 3]) -> Vec<u8> {
    rasterize(
        Shape::of(style, level, false),
        color,
        style.accent_opacity(),
    )
}

/// 没有可读设备时的满格图标：蓝色满填充，不表示具体电量。
#[must_use]
pub fn render_neutral(style: IconStyle) -> Vec<u8> {
    render_style(style, 100, BLUE)
}

/// 低电量提醒的“亮”帧：当前样式的满红图标。
#[must_use]
pub fn render_alert_on(style: IconStyle) -> Vec<u8> {
    render_style(style, 100, RED)
}

/// 低电量提醒的“暗”帧：闪电全透明，圆环只剩红色轨道，电池只剩红色空壳。
#[must_use]
pub fn render_alert_off(style: IconStyle) -> Vec<u8> {
    match style {
        IconStyle::Bolt => blank(),
        IconStyle::Ring | IconStyle::Battery => {
            rasterize(Shape::of(style, 0, true), RED, style.accent_opacity())
        }
    }
}

fn rasterize(shape: Shape, color: [u8; 3], accent_opacity: f32) -> Vec<u8> {
    let unit = shape.unit();
    let side = SUPERSAMPLE as f32;
    let inv_samples = 1.0 / (side * side);
    let src = [
        f32::from(color[0]) / 255.0,
        f32::from(color[1]) / 255.0,
        f32::from(color[2]) / 255.0,
    ];
    let mut rgba = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    for py in 0..ICON_SIZE {
        for px in 0..ICON_SIZE {
            let mut acc = [0.0f32; 4];
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let x = (px as f32 + (sx as f32 + 0.5) / side) * unit;
                    let y = (py as f32 + (sy as f32 + 0.5) / side) * unit;
                    let sample = match shape.ink(x, y) {
                        Ink::None => [0.0, 0.0, 0.0, 0.0],
                        Ink::Accent => [
                            src[0] * accent_opacity,
                            src[1] * accent_opacity,
                            src[2] * accent_opacity,
                            accent_opacity,
                        ],
                        Ink::Fill => [src[0], src[1], src[2], 1.0],
                    };
                    for channel in 0..4 {
                        acc[channel] += sample[channel];
                    }
                }
            }

            let alpha = acc[3] * inv_samples;
            if alpha <= 0.0 {
                continue;
            }
            let index = ((py * ICON_SIZE + px) * 4) as usize;
            for channel in 0..3 {
                let straight = acc[channel] * inv_samples / alpha;
                rgba[index + channel] = (straight * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            rgba[index + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }

    rgba
}

/// 闪烁用的全透明帧。Windows GDI 会把全 0 alpha 当作无 alpha 通道渲染成黑块，
/// 因此整帧保留 alpha=1（约 0.4% 不透明度，肉眼不可见）。
#[must_use]
pub fn blank() -> Vec<u8> {
    let mut rgba = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    for alpha in rgba.iter_mut().skip(3).step_by(4) {
        *alpha = 1;
    }
    rgba
}

fn inside_polygon(x: f32, y: f32) -> bool {
    let mut inside = false;
    let mut previous = BOLT.len() - 1;
    for (current, &(xi, yi)) in BOLT.iter().enumerate() {
        let (xj, yj) = BOLT[previous];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

fn distance_to_bolt(x: f32, y: f32) -> f32 {
    let mut nearest = f32::MAX;
    for (index, &start) in BOLT.iter().enumerate() {
        let end = BOLT[(index + 1) % BOLT.len()];
        nearest = nearest.min(distance_to_segment(x, y, start, end));
    }
    nearest
}

fn distance_to_segment(px: f32, py: f32, start: (f32, f32), end: (f32, f32)) -> f32 {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq == 0.0 {
        0.0
    } else {
        (((px - start.0) * dx + (py - start.1) * dy) / length_sq).clamp(0.0, 1.0)
    };
    let cx = start.0 + dx * t;
    let cy = start.1 + dy * t;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

fn rounded_rect_contains(x: f32, y: f32, rect: (f32, f32, f32, f32, f32)) -> bool {
    let (rx, ry, w, h, r) = rect;
    let r = r.min(w / 2.0).min(h / 2.0);
    let left = rx + r;
    let right = (rx + w - r).max(left);
    let top = ry + r;
    let bottom = (ry + h - r).max(top);
    let cx = x.clamp(left, right);
    let cy = y.clamp(top, bottom);
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r + f32::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(rgba: &[u8], x: u32, y: u32) -> u8 {
        rgba[((y * ICON_SIZE + x) * 4 + 3) as usize]
    }

    fn colored_pixels(rgba: &[u8]) -> usize {
        rgba.iter().skip(3).step_by(4).filter(|&&a| a > 8).count()
    }

    #[test]
    fn parses_styles() {
        assert_eq!(IconStyle::parse("bolt"), IconStyle::Bolt);
        assert_eq!(IconStyle::parse("ring"), IconStyle::Ring);
        assert_eq!(IconStyle::parse("battery"), IconStyle::Battery);
        assert_eq!(IconStyle::parse("RING"), IconStyle::Bolt);
        assert_eq!(IconStyle::parse(""), IconStyle::Bolt);
        assert_eq!(IconStyle::parse("foo"), IconStyle::Bolt);
    }

    #[test]
    fn quantizes_to_five_percent() {
        assert_eq!(quantize(0), 0);
        assert_eq!(quantize(4), 0);
        assert_eq!(quantize(5), 5);
        assert_eq!(quantize(99), 95);
        assert_eq!(quantize(100), 100);
        assert_eq!(quantize(150), 100);
    }

    #[test]
    fn renders_all_styles_and_levels() {
        for style in [IconStyle::Bolt, IconStyle::Ring, IconStyle::Battery] {
            for level in [0, 50, 100] {
                let rgba = render_style(style, level, GREEN);
                assert_eq!(rgba.len(), 16_384);
                assert!(colored_pixels(&rgba) > 0, "{style:?} {level} is empty");
            }
        }
    }

    #[test]
    fn fill_grows_with_level() {
        for style in [IconStyle::Bolt, IconStyle::Ring, IconStyle::Battery] {
            let low = colored_pixels(&render_style(style, 20, GREEN));
            let mid = colored_pixels(&render_style(style, 50, GREEN));
            let high = colored_pixels(&render_style(style, 100, GREEN));
            assert!(high > mid && mid > low, "{style:?}: {low}/{mid}/{high}");
        }
    }

    #[test]
    fn zero_level_keeps_outline() {
        for style in [IconStyle::Bolt, IconStyle::Ring, IconStyle::Battery] {
            let empty = render_style(style, 0, RED);
            let full = render_style(style, 100, RED);
            assert!(
                colored_pixels(&empty) > 0,
                "{style:?} empty frame is invisible"
            );
            assert!(colored_pixels(&empty) < colored_pixels(&full));
        }
    }

    #[test]
    fn ring_geometry_has_center_hole_and_top_band() {
        let rgba = render_style(IconStyle::Ring, 100, GREEN);
        assert_eq!(alpha_at(&rgba, 32, 32), 0);
        assert!(alpha_at(&rgba, 32, 8) > 128);
    }

    #[test]
    fn battery_geometry_has_shell_and_fill() {
        let full = render_style(IconStyle::Battery, 100, GREEN);
        let empty = render_style(IconStyle::Battery, 0, GREEN);
        assert!(alpha_at(&full, 56, 32) > 128);
        assert!(alpha_at(&full, 32, 32) > 128);
        assert_eq!(alpha_at(&empty, 32, 32), 0);
    }

    #[test]
    fn ring_arc_follows_clockwise_sweep() {
        let rgba = render_style(IconStyle::Ring, 60, GREEN);
        assert!(alpha_at(&rgba, 32, 8) > 200, "top should be filled");
        assert!(alpha_at(&rgba, 56, 32) > 200, "right should be filled");
        assert!(alpha_at(&rgba, 32, 56) > 200, "bottom should be filled");
        assert!(alpha_at(&rgba, 8, 32) < 160, "left should still be track");
    }

    #[test]
    fn battery_fill_width_tracks_level() {
        let twenty = render_style(IconStyle::Battery, 20, GREEN);
        assert!(alpha_at(&twenty, 15, 32) > 200, "20% should reach x=15");
        assert_eq!(alpha_at(&twenty, 25, 32), 0, "20% should stop before x=25");

        let sixty = render_style(IconStyle::Battery, 60, GREEN);
        assert!(alpha_at(&sixty, 30, 32) > 200, "60% should reach x=30");
        assert_eq!(alpha_at(&sixty, 40, 32), 0, "60% should stop before x=40");
    }

    #[test]
    fn neutral_is_blue_and_full() {
        let rgba = render_neutral(IconStyle::Battery);
        let index = ((32 * ICON_SIZE + 32) * 4) as usize;
        assert!(rgba[index + 2] > rgba[index]);
        assert!(rgba[index + 2] > rgba[index + 1]);
    }

    #[test]
    fn blank_keeps_alpha_one() {
        let rgba = blank();
        assert_eq!(rgba.len(), 16_384);
        for alpha in rgba.iter().skip(3).step_by(4) {
            assert_eq!(*alpha, 1);
        }
    }

    #[test]
    fn alert_frames_are_shape_specific() {
        let bolt_on = render_alert_on(IconStyle::Bolt);
        let bolt_off = render_alert_off(IconStyle::Bolt);
        assert!(colored_pixels(&bolt_on) > 0);
        assert_eq!(bolt_off, blank());

        for style in [IconStyle::Ring, IconStyle::Battery] {
            let on = render_alert_on(style);
            let off = render_alert_off(style);
            assert!(colored_pixels(&on) > colored_pixels(&off), "{style:?}");
            assert!(colored_pixels(&off) > 0, "{style:?} off frame is invisible");
        }
    }
}
