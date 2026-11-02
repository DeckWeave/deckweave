//! Original short PCM cues. Scaling is per buffer and never changes master volume.
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Cue {
    Complete,
    Notice,
    Warning,
}
pub fn wave(cue: Cue, volume: u8) -> Result<Vec<u8>, String> {
    if volume > 100 {
        return Err("Sound volume must be 0..100".into());
    }
    let (notes, seconds): (&[f64], f64) = match cue {
        Cue::Complete => (&[523.25, 659.25, 783.99], 0.72),
        Cue::Notice => (&[880., 1174.66], 0.32),
        Cue::Warning => (&[440., 349.23], 0.55),
    };
    let rate = 24000u32;
    let count = (seconds * rate as f64) as u32;
    let size = count * 2;
    let mut out = Vec::with_capacity((44 + size) as usize);
    out.extend(b"RIFF");
    out.extend((36 + size).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend((rate * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(size.to_le_bytes());
    for n in 0..count {
        let t = n as f64 / rate as f64;
        let value = notes
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let local = t - i as f64 * 0.065;
                if local <= 0. {
                    0.
                } else {
                    (TAU * f * local).sin() * (local / 0.012).min(1.) * (-7. * local).exp()
                }
            })
            .sum::<f64>()
            / notes.len() as f64;
        let tail = ((seconds - t) / 0.025).clamp(0., 1.);
        let pcm = (value * tail * 0.32 * volume as f64 / 100. * i16::MAX as f64).round() as i16;
        out.extend(pcm.to_le_bytes());
    }
    Ok(out)
}
/// Blocking short playback; callers must use the single dedicated sound worker.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn play(cue: Cue, volume: u8) -> Result<(), String> {
    let buffer = wave(cue, volume)?;
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
    // SAFETY: SND_MEMORY treats this pointer as a complete RIFF byte buffer. SYNC
    // retains no pointer after return. The private buffer lives for the entire call.
    if unsafe {
        PlaySoundW(
            buffer.as_ptr().cast(),
            std::ptr::null_mut(),
            SND_MEMORY | SND_NODEFAULT | SND_SYNC,
        )
    } == 0
    {
        return Err("Windows could not play the notification sound".into());
    }
    Ok(())
}
#[cfg(not(windows))]
pub fn play(_: Cue, _: u8) -> Result<(), String> {
    Err("Notification playback is not yet supported on this platform".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_bounded_pcm_with_per_buffer_volume_and_silent_zero() {
        for cue in [Cue::Complete, Cue::Notice, Cue::Warning] {
            let full = wave(cue, 100).unwrap();
            let silent = wave(cue, 0).unwrap();
            let half = wave(cue, 50).unwrap();
            assert_eq!(&full[..4], b"RIFF");
            assert_eq!(&full[8..16], b"WAVEfmt ");
            assert!(full.len() < 40000);
            assert!(silent[44..].iter().all(|b| *b == 0));
            let amplitude = |b: &[u8]| {
                b[44..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| i16::from_le_bytes([p[0], p[1]]).unsigned_abs())
                    .max()
                    .unwrap()
            };
            assert!(amplitude(&full) <= 11000);
            assert!(amplitude(&half) <= amplitude(&full) / 2 + 1);
        }
        assert!(wave(Cue::Notice, 101).is_err());
    }
}
