//! 托盘闪电图标的纯函数栅格化。
//!
//! 形状取自 `bolt-all-states.html`：六个顶点的闪电，按电量把填充从底部裁切，
//! 描边使用同色 63% 透明度；颜色按设置页三档区间取绿 / 黄 / 红。
//! 纯 CPU 栅格化 + 4× 超采样，不引入图像处理依赖。

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
const SUPERSAMPLE: u32 = 4;
const LEVEL_STEP: u8 = 5;

pub const GREEN: [u8; 3] = [0x3c, 0xe2, 0x8a];
pub const YELLOW: [u8; 3] = [0xf0, 0xad, 0x4e];
pub const RED: [u8; 3] = [0xff, 0x4d, 0x4f];
pub const BLUE: [u8; 3] = [0x5b, 0x9c, 0xf5];

/// 电量档位，与设置页「电量充足 / 正常使用 / 电量不足」一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// 渲染指定电量的闪电；`color` 决定档位色，填充高度由 5% 量化后的电量决定。
#[must_use]
pub fn render(level: u8, color: [u8; 3]) -> Vec<u8> {
    let level = f32::from(quantize(level));
    let clip_top = BOTTOM - (BOTTOM - TOP) * level / 100.0;
    let scale = 24.0 / ICON_SIZE as f32;
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
                    let x = (px as f32 + (sx as f32 + 0.5) / side) * scale;
                    let y = (py as f32 + (sy as f32 + 0.5) / side) * scale;
                    let fill = inside_polygon(x, y) && y >= clip_top;
                    let stroke = distance_to_bolt(x, y) <= STROKE_HALF_WIDTH;

                    let mut sample = [0.0f32; 4];
                    if fill {
                        sample = [src[0], src[1], src[2], 1.0];
                    }
                    if stroke {
                        for channel in 0..3 {
                            sample[channel] = src[channel] * STROKE_OPACITY
                                + sample[channel] * (1.0 - STROKE_OPACITY);
                        }
                        sample[3] = STROKE_OPACITY + sample[3] * (1.0 - STROKE_OPACITY);
                    }
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

#[must_use]
pub fn render_tier(level: u8, tier: Tier) -> Vec<u8> {
    render(level, tier.color())
}

/// 没有可读设备时的满格闪电：蓝色满填充，不表示具体电量。
#[must_use]
pub fn render_neutral() -> Vec<u8> {
    render(100, BLUE)
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
