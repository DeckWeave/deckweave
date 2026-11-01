// SPDX-License-Identifier: GPL-3.0-or-later
// ACK decoder based on DeckWeave/plugins/device-n4pro/src/report.rs.
// Input numbering verified with owner captures 2026-11-01; BAT output uses a different map.
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Input {
    Key { key: u8, pressed: bool },
    Touch { x: u16, pressed: bool },
    Rotate { dial: u8, ticks: i32 },
    DialClick { dial: u8 },
    Pointer { x: u16, y: u16 },
    Swipe { right: bool },
    Unknown { code: u8, state: u8 },
}

pub fn decode(bytes: &[u8]) -> Option<Input> {
    if !(11..=1024).contains(&bytes.len()) || bytes.get(..3) != Some(b"ACK") {
        return None;
    }
    if bytes.get(..7) == Some(b"ACK\0ARX") {
        return (bytes.len() >= 14).then(|| Input::Pointer {
            x: u16::from_be_bytes([bytes[10], bytes[11]]),
            y: u16::from_be_bytes([bytes[12], bytes[13]]),
        });
    }
    if bytes.get(..9) != Some(b"ACK\0\0OK\0\0") {
        return None;
    }
    let (code, state) = (bytes[9], bytes[10]);
    if [0, 1, 2].contains(&state) {
        let pressed = state == 1;
        if (1..=10).contains(&code) {
            return Some(Input::Key { key: code, pressed });
        }
    }
    // Captures: 00/01 starts a contact; 00/00 or 40..43/00 ends it.
    // The trailing X coordinate, not the four firmware zone hints, addresses our layout.
    if (code == 0 && [0, 1].contains(&state)) || ((0x40..=0x43).contains(&code) && state == 0) {
        return (bytes.len() >= 13).then(|| Input::Touch {
            x: u16::from_be_bytes([bytes[11], bytes[12]]),
            pressed: state == 1,
        });
    }
    if state == 0 {
        let pair = match code {
            0xa0 => Some((1, -1)),
            0xa1 => Some((1, 1)),
            0x50 => Some((2, -1)),
            0x51 => Some((2, 1)),
            0x90 => Some((3, -1)),
            0x91 => Some((3, 1)),
            0x70 => Some((4, -1)),
            0x71 => Some((4, 1)),
            _ => None,
        };
        if let Some((dial, ticks)) = pair {
            return Some(Input::Rotate { dial, ticks });
        }
    }
    if state == 1 {
        let dial = match code {
            0x37 => Some(1),
            0x35 => Some(2),
            0x33 => Some(3),
            0x36 => Some(4),
            _ => None,
        };
        if let Some(dial) = dial {
            return Some(Input::DialClick { dial });
        }
    }
    // Gesture codes are recorded, not automatically converted into a second touch tap.
    if [0, 1].contains(&state) && [0x38, 0x39].contains(&code) {
        return Some(Input::Swipe {
            right: code == 0x39,
        });
    }
    Some(Input::Unknown { code, state })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(code: u8, state: u8) -> Vec<u8> {
        let mut b = b"ACK\0\0OK\0\0".to_vec();
        b.extend([code, state]);
        b
    }
    #[test]
    fn physical_input_numbers_are_independent_of_bat_output_addresses() {
        for key in 1..=10 {
            let raw = key;
            assert_eq!(
                decode(&packet(raw, 1)),
                Some(Input::Key { key, pressed: true })
            );
            for state in [0, 2] {
                assert_eq!(
                    decode(&packet(raw, state)),
                    Some(Input::Key {
                        key,
                        pressed: false
                    })
                );
            }
        }
        assert_eq!(
            decode(&packet(11, 1)),
            Some(Input::Unknown { code: 11, state: 1 })
        );
        assert_eq!(
            decode(&packet(11, 255)),
            Some(Input::Unknown {
                code: 11,
                state: 255
            })
        );
    }
    #[test]
    fn malformed_reports_never_reuse_previous_input_or_invent_contacts() {
        let b = b"ACK\0ARX\0\0\0\x03\x1f\0\x77";
        for n in 0..14 {
            assert_eq!(decode(&b[..n]), None);
        }
        assert_eq!(decode(b), Some(Input::Pointer { x: 799, y: 119 }));
        assert_eq!(decode(&[0; 1025]), None);
        assert_eq!(decode(b"ACK\0VER\0\0\x01\x01"), None);
        assert_eq!(decode(&packet(0, 1)), None);
        assert!(matches!(
            decode(&packet(0x40, 1)),
            Some(Input::Unknown { .. })
        ));
    }
    #[test]
    fn repeated_detents_survive_and_click_does_not_claim_dial_release_or_hold() {
        for _ in 0..4 {
            assert_eq!(
                decode(&packet(0xa1, 0)),
                Some(Input::Rotate { dial: 1, ticks: 1 })
            );
        }
        assert_eq!(decode(&packet(0x37, 1)), Some(Input::DialClick { dial: 1 }));
        assert_eq!(
            decode(&packet(0x37, 0)),
            Some(Input::Unknown {
                code: 0x37,
                state: 0
            })
        );
    }
}
