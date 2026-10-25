//! Kurze Feedback-Töne. Werden einmal synthetisch als WAV im Speicher erzeugt
//! und asynchron per `PlaySoundW(SND_MEMORY)` abgespielt (kein Thread, keine Datei).

use std::sync::Arc;

use parking_lot::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

use crate::settings::SoundSettings;

#[derive(Debug, Clone, Copy)]
pub enum SoundKind {
    Saved,
    RecordStart,
    RecordStop,
    Error,
}

#[derive(Clone)]
pub struct Sound {
    inner: Arc<Mutex<Bank>>,
}

struct Bank {
    volume: f32,
    sounds: [Arc<Vec<u8>>; 4],
    /// PlaySound liest asynchron aus dem Speicher: alte Puffer bleiben gültig.
    graveyard: Vec<Arc<Vec<u8>>>,
}

const RATE: u32 = 44_100;

/// (Frequenzen in Hz, Länge je Ton in ms)
fn recipe(kind: SoundKind) -> (&'static [f32], u32) {
    match kind {
        SoundKind::Saved => (&[880.0, 1318.5], 70),
        SoundKind::RecordStart => (&[660.0, 990.0], 60),
        SoundKind::RecordStop => (&[990.0, 660.0], 60),
        SoundKind::Error => (&[220.0, 196.0], 90),
    }
}

fn synth(kind: SoundKind, volume: f32) -> Vec<u8> {
    let (notes, ms) = recipe(kind);
    let per = (RATE * ms / 1000) as usize;
    let total = per * notes.len();
    let mut pcm: Vec<i16> = Vec::with_capacity(total);
    for (n, &f) in notes.iter().enumerate() {
        for i in 0..per {
            let t = i as f32 / RATE as f32;
            // Weiche Hüllkurve gegen Knackser.
            let attack = (i as f32 / (RATE as f32 * 0.004)).min(1.0);
            let release = ((per - i) as f32 / (RATE as f32 * 0.02)).min(1.0);
            let env = attack * release * if n + 1 == notes.len() { 1.0 } else { 0.9 };
            let s = (2.0 * std::f32::consts::PI * f * t).sin() * 0.35 * env * volume;
            pcm.push((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
    }
    wav(&pcm)
}

fn wav(pcm: &[i16]) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&RATE.to_le_bytes());
    v.extend_from_slice(&(RATE * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

fn bank(volume: f32) -> [Arc<Vec<u8>>; 4] {
    [
        Arc::new(synth(SoundKind::Saved, volume)),
        Arc::new(synth(SoundKind::RecordStart, volume)),
        Arc::new(synth(SoundKind::RecordStop, volume)),
        Arc::new(synth(SoundKind::Error, volume)),
    ]
}

impl Sound {
    pub fn new(volume: f32) -> Self {
        Sound { inner: Arc::new(Mutex::new(Bank { volume, sounds: bank(volume), graveyard: Vec::new() })) }
    }

    pub fn set_volume(&self, volume: f32) {
        let mut b = self.inner.lock();
        if (b.volume - volume).abs() < 0.001 {
            return;
        }
        let old = std::mem::replace(&mut b.sounds, bank(volume));
        b.graveyard.extend(old);
        // Nur die letzten Puffer behalten; ältere sind längst ausgespielt.
        let len = b.graveyard.len();
        if len > 16 {
            b.graveyard.drain(..len - 16);
        }
        b.volume = volume;
    }

    pub fn play(&self, kind: SoundKind, cfg: &SoundSettings) {
        if !cfg.enabled || cfg.volume <= 0.0 {
            return;
        }
        let b = self.inner.lock();
        let buf = &b.sounds[kind as usize];
        // SAFETY: Der Puffer lebt in der Bank (bzw. im Graveyard) länger als die Wiedergabe.
        unsafe {
            let _ = PlaySoundW(PCWSTR(buf.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let w = synth(SoundKind::Saved, 0.5);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        let len = u32::from_le_bytes([w[40], w[41], w[42], w[43]]) as usize;
        assert_eq!(len + 44, w.len());
    }
}
