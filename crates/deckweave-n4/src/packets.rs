// SPDX-License-Identifier: GPL-3.0-or-later
// Volatile BAT/STP and session packet layout adapted from old DeckWeave N4 driver.
pub const REPORT_SIZE: usize = 1025;
pub const HEARTBEAT_MS: u64 = 8_000;
pub fn command(payload: &[u8]) -> [u8; REPORT_SIZE] {
    assert!(payload.len() <= 1019);
    let mut report = [0; REPORT_SIZE];
    report[1..6].copy_from_slice(b"CRT\0\0");
    report[6..6 + payload.len()].copy_from_slice(payload);
    report
}
pub fn wake() -> [u8; REPORT_SIZE] {
    command(b"DIS")
}
pub fn heartbeat() -> [u8; REPORT_SIZE] {
    command(b"CONNECT")
}
pub fn clear_background() -> [u8; REPORT_SIZE] {
    command(b"BGCLE\x03")
}
/// Matches the established N4 driver: perceptual slider percentage to device percentage.
fn brightness_percent(value: u8) -> u8 {
    ((f64::from(value.min(100)) / 100.0).powf(0.75) * 100.0).round() as u8
}
pub fn screen_brightness(value: u8) -> [u8; REPORT_SIZE] {
    command(&[b'L', b'I', b'G', 0, 0, brightness_percent(value)])
}
pub fn dial_brightness(value: u8) -> [u8; REPORT_SIZE] {
    command(&[b'L', b'B', b'L', b'I', b'G', brightness_percent(value)])
}
pub fn vibration(enabled: bool) -> [u8; REPORT_SIZE] {
    command(&[
        b'Q',
        b'U',
        b'C',
        b'M',
        b'D',
        0x11,
        0x11,
        0x11,
        if enabled { 0x11 } else { 0xff },
        0x11,
        0x11,
    ])
}
pub fn key_image(
    key: u8,
    jpeg: &[u8],
    mut write: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    if !(1..=10).contains(&key) || jpeg.is_empty() || jpeg.len() > 10240 {
        return Err("Invalid N4 key frame".into());
    }
    let mut header = b"BAT".to_vec();
    header.extend((jpeg.len() as u32).to_be_bytes());
    header.push(if key <= 5 { key + 10 } else { key });
    write(&command(&header))?;
    for chunk in jpeg.chunks(1024) {
        let mut report = [0; REPORT_SIZE];
        report[1..1 + chunk.len()].copy_from_slice(chunk);
        write(&report)?;
    }
    write(&command(b"STP"))
}

/// Volatile 800x128 touch-strip framebuffer, adapted from old transfer_region.
pub fn strip_image(
    jpeg: &[u8],
    mut write: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    if jpeg.is_empty() || jpeg.len() > 128 * 1024 {
        return Err("Invalid N4 strip frame".into());
    }
    let mut header = b"BGPIC".to_vec();
    header.extend((jpeg.len() as u32).to_be_bytes());
    for value in [0u16, 0, 800, 128] {
        header.extend(value.to_be_bytes());
    }
    header.extend([0, 0]);
    write(&command(&header))?;
    for chunk in jpeg.chunks(1024) {
        let mut report = [0; REPORT_SIZE];
        report[1..1 + chunk.len()].copy_from_slice(chunk);
        write(&report)?;
    }
    write(&command(b"STP"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hardware_settings_match_existing_n4_driver_without_changing_image_packets() {
        assert_eq!(&screen_brightness(50)[6..12], b"LIG\0\0\x3b");
        assert_eq!(&dial_brightness(50)[6..12], b"LBLIG\x3b");
        for (value, wire) in [(0, 0), (1, 3), (25, 35), (75, 81), (100, 100), (255, 100)] {
            assert_eq!(screen_brightness(value)[11], wire);
            assert_eq!(dial_brightness(value)[11], wire);
        }
        assert_eq!(&vibration(false)[6..17], b"QUCMD\x11\x11\x11\xff\x11\x11");
        assert_eq!(&vibration(true)[6..17], b"QUCMD\x11\x11\x11\x11\x11\x11");
        assert!(vibration(false)[17..].iter().all(|b| *b == 0));
        assert_eq!(&vibration(false)[1..6], b"CRT\0\0");
    }
    #[test]
    fn strip_uses_bounded_volatile_region_and_aborts_on_incomplete_transaction() {
        let mut reports = vec![];
        strip_image(&[1, 2, 3], |r| {
            reports.push(r.to_vec());
            Ok(())
        })
        .unwrap();
        assert_eq!(
            &reports[0][6..25],
            b"BGPIC\0\0\0\x03\0\0\0\0\x03\x20\0\x80\0\0"
        );
        assert_eq!(&reports[2][6..9], b"STP");
        assert!(strip_image(&vec![0; 128 * 1024 + 1], |_| panic!()).is_err());
        let mut calls = 0;
        assert!(
            strip_image(&[1, 2, 3], |_| {
                calls += 1;
                if calls == 2 {
                    Err("failed".into())
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        assert_eq!(calls, 2);
    }
    #[test]
    fn whole_image_transaction_is_bounded_ordered_and_volatile() {
        let mut reports = vec![];
        key_image(1, &vec![42; 1025], |r| {
            reports.push(r.to_vec());
            Ok(())
        })
        .unwrap();
        assert_eq!(reports.len(), 4);
        assert_eq!(&reports[0][6..14], b"BAT\0\0\x04\x01\x0b");
        assert_eq!(&reports[1][1..], &[42; 1024]);
        assert_eq!(reports[2][1], 42);
        assert_eq!(&reports[3][6..9], b"STP");
        assert!(
            reports
                .iter()
                .all(|r| r.len() == 1025 && !r.windows(3).any(|w| w == b"LOG"))
        );
    }
    #[test]
    fn failed_write_aborts_before_refresh_and_bad_key_writes_nothing() {
        let mut count = 0;
        assert!(
            key_image(2, &[1, 2], |_| {
                count += 1;
                Err("Disconnected".into())
            })
            .is_err()
        );
        assert_eq!(count, 1);
        assert!(key_image(0, &[1], |_| panic!()).is_err());
    }
}
