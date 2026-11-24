//! 请求/回包的字节变换与校验和。
//!
//! 偏移口径与 profile 的 `response.*.offset` 一致：都按含 report ID 的完整
//! HID 报告缓冲计算。校验失败只代表这一帧无效，不直接判设备错误。

use std::borrow::Cow;

use super::profiles::{ByteTransform, ChecksumAlgo, ChecksumEndian, ChecksumOut, ChecksumSpec};

#[must_use]
pub fn transform(frame: &[u8], mode: ByteTransform) -> Cow<'_, [u8]> {
    match mode {
        ByteTransform::None => Cow::Borrowed(frame),
        ByteTransform::Invert => Cow::Owned(frame.iter().map(|byte| !byte).collect()),
    }
}

pub fn apply_request_checksum(frame: &mut [u8], spec: &ChecksumSpec) -> Result<(), String> {
    if spec.zero_target {
        zero_target(frame, spec)?;
    }
    let value = computed_value(frame, spec)?;
    write_value(frame, spec, value);
    Ok(())
}

#[must_use]
pub fn verify_checksum(frame: &[u8], spec: &ChecksumSpec) -> bool {
    let expected = read_value(frame, spec);
    let Some(expected) = expected else {
        return false;
    };
    let computed = if spec.zero_target {
        let mut copy = frame.to_vec();
        if zero_target(&mut copy, spec).is_err() {
            return false;
        }
        computed_value(&copy, spec)
    } else {
        computed_value(frame, spec)
    };
    computed.is_ok_and(|value| value == expected)
}

fn width(algo: ChecksumAlgo) -> usize {
    match algo {
        ChecksumAlgo::Crc16CcittFalse => 2,
        ChecksumAlgo::Sum8 | ChecksumAlgo::Xor8 => 1,
    }
}

fn target_range(spec: &ChecksumSpec, frame_len: usize) -> Option<(usize, usize)> {
    let target = usize::from(spec.target);
    let width = width(spec.algo);
    (target + width <= frame_len).then_some((target, width))
}

fn zero_target(frame: &mut [u8], spec: &ChecksumSpec) -> Result<(), String> {
    let Some((target, width)) = target_range(spec, frame.len()) else {
        return Err("checksum.target 越界".to_string());
    };
    frame[target..target + width].fill(0);
    Ok(())
}

fn computed_value(frame: &[u8], spec: &ChecksumSpec) -> Result<u16, String> {
    let start = usize::from(spec.start);
    let end = usize::from(spec.end);
    if start >= end || end > frame.len() {
        return Err("checksum.start/end 越界".to_string());
    }
    let bytes = &frame[start..end];
    let raw = match spec.algo {
        ChecksumAlgo::Sum8 => bytes
            .iter()
            .fold(0u16, |sum, byte| sum.wrapping_add(u16::from(*byte)) & 0xFF),
        ChecksumAlgo::Xor8 => bytes.iter().fold(0u16, |xor, byte| xor ^ u16::from(*byte)),
        ChecksumAlgo::Crc16CcittFalse => crc16_ccitt_false(bytes),
    };
    let mask = if width(spec.algo) == 2 { 0xFFFF } else { 0xFF };
    let value = match spec.out {
        ChecksumOut::Raw => raw,
        ChecksumOut::Not => !raw & mask,
        ChecksumOut::BaseMinus => {
            (u32::from(spec.base.unwrap_or(0)).wrapping_sub(u32::from(raw)) & u32::from(mask))
                as u16
        }
    };
    Ok(value & mask)
}

fn read_value(frame: &[u8], spec: &ChecksumSpec) -> Option<u16> {
    let (target, width) = target_range(spec, frame.len())?;
    if width == 1 {
        return Some(u16::from(frame[target]));
    }
    let low = u16::from(frame[target]);
    let high = u16::from(frame[target + 1]);
    Some(match spec.endian {
        ChecksumEndian::Little => low | (high << 8),
        ChecksumEndian::Big => (low << 8) | high,
    })
}

fn write_value(frame: &mut [u8], spec: &ChecksumSpec, value: u16) {
    let Some((target, width)) = target_range(spec, frame.len()) else {
        return;
    };
    if width == 1 {
        frame[target] = value as u8;
        return;
    }
    let (low, high) = match spec.endian {
        ChecksumEndian::Little => (value as u8, (value >> 8) as u8),
        ChecksumEndian::Big => ((value >> 8) as u8, value as u8),
    };
    frame[target] = low;
    frame[target + 1] = high;
}

fn crc16_ccitt_false(bytes: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(algo: ChecksumAlgo, out: ChecksumOut, base: Option<u16>, zero: bool) -> ChecksumSpec {
        ChecksumSpec {
            algo,
            start: 0,
            end: 4,
            target: 4,
            endian: ChecksumEndian::Little,
            out,
            base,
            zero_target: zero,
        }
    }

    #[test]
    fn crc16_known_vector() {
        assert_eq!(crc16_ccitt_false(b"123456789"), 0x29B1);
    }

    #[test]
    fn sum8_and_xor8() {
        let frame = [1u8, 2, 3, 0, 0];
        let sum = spec(ChecksumAlgo::Sum8, ChecksumOut::Raw, None, false);
        let mut out = frame;
        apply_request_checksum(&mut out, &sum).unwrap();
        assert_eq!(out[4], 6);

        let xor = spec(ChecksumAlgo::Xor8, ChecksumOut::Raw, None, false);
        let mut out = frame;
        apply_request_checksum(&mut out, &xor).unwrap();
        assert_eq!(out[4], 0);
    }

    #[test]
    fn base_minus_and_zero_target() {
        let mut frame = [0x10u8, 0x20, 0x30, 0x40, 0xEE];
        let s = spec(ChecksumAlgo::Sum8, ChecksumOut::BaseMinus, Some(0x55), true);
        apply_request_checksum(&mut frame, &s).unwrap();
        assert_eq!(frame[4], 0x55u8.wrapping_sub(0x10 + 0x20 + 0x30 + 0x40));
        assert!(verify_checksum(&frame, &s));
    }

    #[test]
    fn invert_round_trip() {
        let frame = [0x00u8, 0x01, 0xFE, 0xFF];
        let inverted = transform(&frame, ByteTransform::Invert).into_owned();
        let restored = transform(&inverted, ByteTransform::Invert).into_owned();
        assert_eq!(restored, frame);
    }

    #[test]
    fn crc16_little_endian_round_trip() {
        let mut frame = [0x21u8, 0xBB, 0x0B, 0x00, 0x00, 0x00];
        let s = ChecksumSpec {
            algo: ChecksumAlgo::Crc16CcittFalse,
            start: 0,
            end: 4,
            target: 4,
            endian: ChecksumEndian::Little,
            out: ChecksumOut::Raw,
            base: None,
            zero_target: false,
        };
        apply_request_checksum(&mut frame, &s).unwrap();
        assert!(verify_checksum(&frame, &s));
        frame[4] ^= 0xFF;
        assert!(!verify_checksum(&frame, &s));
    }
}
