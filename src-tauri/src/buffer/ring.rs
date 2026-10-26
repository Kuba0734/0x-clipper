//! Replay-Ringpuffer für **encodierte** Pakete (Video + Audio).
//!
//! - Zeit- und Byte-Limit; verdrängt wird immer eine ganze GOP, damit der Puffer
//!   stets mit einem Video-Keyframe beginnt.
//! - Keine Allokation pro Paket im eingeschwungenen Zustand: verdrängte Pakete gehen
//!   zurück in einen Pool und werden samt `Vec`-Kapazität wiederverwendet.
//! - `snapshot` klont nur `Arc`s; der MuxWorker schreibt danach ohne Lock.
//!
//! Alle Zeitstempel sind Mikrosekunden auf der gemeinsamen QPC-Uhr.

use std::collections::VecDeque;
use std::sync::Arc;

pub const VIDEO_STREAM: u8 = 0;

/// Pool-Obergrenze je Stream-Art. Im Normalbetrieb wird eine GOP verdrängt und sofort
/// wiederverwendet; mehr freie Pakete würden nur ungenutzt RAM halten (z. B. nach `clear`).
const POOL_MAX: usize = 256;
/// Unterhalb dieser Größe lohnt sich das Verkleinern recycelter Puffer nicht.
const MIN_SLACK: usize = 16 * 1024;

fn pool_index(stream: u8) -> usize {
    usize::from(stream != VIDEO_STREAM)
}

#[derive(Debug, Default, Clone)]
pub struct Packet {
    /// 0 = Video, 1.. = Audiospuren.
    pub stream: u8,
    pub pts: i64,
    pub dts: i64,
    pub duration: i64,
    pub key: bool,
    pub data: Vec<u8>,
}

impl Packet {
    pub fn is_video_key(&self) -> bool {
        self.stream == VIDEO_STREAM && self.key
    }
}

/// Wiederverwendbare Pakete. `Arc::get_mut` garantiert, dass niemand (z. B. ein
/// laufender Snapshot) das Paket noch hält, bevor es überschrieben wird.
#[derive(Debug, Default)]
pub struct PacketPool {
    free: Vec<Arc<Packet>>,
    max_free: usize,
}

impl PacketPool {
    pub fn new(max_free: usize) -> Self {
        PacketPool { free: Vec::with_capacity(max_free), max_free }
    }

    fn take(&mut self) -> Arc<Packet> {
        self.free.pop().unwrap_or_default()
    }

    fn give(&mut self, mut p: Arc<Packet>) {
        if self.free.len() < self.max_free && Arc::get_mut(&mut p).is_some() {
            self.free.push(p);
        }
    }

    pub fn len(&self) -> usize {
        self.free.len()
    }

    pub fn is_empty(&self) -> bool {
        self.free.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_duration_us: i64,
    pub max_bytes: usize,
}

#[derive(Debug)]
pub struct ReplayRingBuffer {
    packets: VecDeque<Arc<Packet>>,
    /// PTS der Video-Keyframes im Puffer, aufsteigend.
    keyframes: VecDeque<i64>,
    bytes: usize,
    limits: Limits,
    /// Getrennte Pools für Video (0) und Audio (1): sonst behält ein kleines Audio-Paket
    /// die Kapazität eines großen Keyframes.
    pools: [PacketPool; 2],
    latest_pts: i64,
    first_pts: Option<i64>,
}

/// Ergebnis eines Snapshots: beginnt mit einem Video-Keyframe.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub packets: Vec<Arc<Packet>>,
    pub start_pts: i64,
    pub end_pts: i64,
}

impl Snapshot {
    pub fn duration_us(&self) -> i64 {
        self.end_pts - self.start_pts
    }
}

impl ReplayRingBuffer {
    pub fn new(limits: Limits) -> Self {
        // Kapazität grob für 10 min 60 fps + Audio, damit VecDeque nicht wächst.
        ReplayRingBuffer {
            packets: VecDeque::with_capacity(4096),
            keyframes: VecDeque::with_capacity(128),
            bytes: 0,
            limits,
            pools: [PacketPool::new(POOL_MAX), PacketPool::new(POOL_MAX)],
            latest_pts: 0,
            first_pts: None,
        }
    }

    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
        self.evict();
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Wie `push`, gibt aber zusätzlich eine Referenz auf das gespeicherte Paket zurück
    /// (für eine parallel laufende Aufnahme). `None`, wenn das Paket verworfen wurde.
    pub fn push_shared(
        &mut self,
        stream: u8,
        pts: i64,
        dts: i64,
        duration: i64,
        key: bool,
        data: &[u8],
    ) -> Option<Arc<Packet>> {
        self.push(stream, pts, dts, duration, key, data);
        // Ein angenommenes Paket liegt immer hinten (Verdrängung passiert nur vorne).
        self.packets.back().filter(|p| p.pts == pts && p.stream == stream).cloned()
    }

    /// Kopiert ein Paket in den Puffer (Pool-Recycling, keine Allokation im Normalfall).
    pub fn push(&mut self, stream: u8, pts: i64, dts: i64, duration: i64, key: bool, data: &[u8]) {
        let mut slot = self.pools[pool_index(stream)].take();
        let p = match Arc::get_mut(&mut slot) {
            Some(p) => p,
            // Pool liefert nur exklusive Pakete; dieser Zweig ist praktisch unerreichbar.
            None => {
                slot = Arc::new(Packet::default());
                match Arc::get_mut(&mut slot) {
                    Some(p) => p,
                    None => return,
                }
            }
        };
        p.stream = stream;
        p.pts = pts;
        p.dts = dts;
        p.duration = duration;
        p.key = key;
        // Recycelte Puffer, die viel größer als nötig sind (alter Keyframe für ein P-Frame),
        // werden ersetzt. Sonst wächst jede Pool-Kapazität auf das Maximum aller Pakete.
        if p.data.capacity() > data.len().max(MIN_SLACK) * 4 {
            p.data = Vec::with_capacity(data.len());
        }
        p.data.clear();
        p.data.extend_from_slice(data);
        self.push_arc(slot);
    }

    /// Nimmt ein bereits fertiges Paket auf (z. B. aus Tests oder Benchmarks).
    pub fn push_arc(&mut self, p: Arc<Packet>) {
        if p.stream == VIDEO_STREAM {
            // Video vor dem ersten Keyframe ist nicht dekodierbar.
            if self.keyframes.is_empty() && !p.key {
                self.pools[pool_index(p.stream)].give(p);
                return;
            }
            if p.key {
                self.keyframes.push_back(p.pts);
            }
            self.latest_pts = self.latest_pts.max(p.pts + p.duration);
            self.first_pts.get_or_insert(p.pts);
        } else if self.keyframes.is_empty() {
            // Audio vor dem ersten Video-Keyframe würde nie gespeichert.
            self.pools[pool_index(p.stream)].give(p);
            return;
        }
        self.bytes += p.data.capacity();
        self.packets.push_back(p);
        self.evict();
    }

    /// Verdrängt ganze GOPs, solange der zweite Keyframe noch vor dem Zeitfenster liegt
    /// oder das Byte-Limit überschritten ist.
    fn evict(&mut self) {
        let window_start = self.latest_pts - self.limits.max_duration_us;
        while self.keyframes.len() >= 2 {
            let second = self.keyframes[1];
            let too_old = second <= window_start;
            let too_big = self.bytes > self.limits.max_bytes;
            if !too_old && !too_big {
                break;
            }
            self.drop_first_gop(second);
        }
        // Eine einzige riesige GOP über dem doppelten Limit: lieber leeren als unbegrenzt wachsen.
        if self.keyframes.len() == 1 && self.bytes > self.limits.max_bytes.saturating_mul(2) {
            self.clear();
        }
    }

    fn drop_first_gop(&mut self, next_key_pts: i64) {
        self.keyframes.pop_front();
        while let Some(front) = self.packets.front() {
            if front.is_video_key() && front.pts == next_key_pts {
                break;
            }
            if let Some(p) = self.packets.pop_front() {
                self.bytes -= p.data.capacity();
                self.pools[pool_index(p.stream)].give(p);
            }
        }
        self.first_pts = self.packets.front().map(|p| p.pts);
    }

    pub fn clear(&mut self) {
        while let Some(p) = self.packets.pop_front() {
            self.pools[pool_index(p.stream)].give(p);
        }
        self.keyframes.clear();
        self.bytes = 0;
        self.first_pts = None;
        self.latest_pts = 0;
    }

    /// Letzte `duration_us` ab dem letzten Keyframe **vor** `jetzt - duration`.
    pub fn snapshot(&self, duration_us: i64) -> Option<Snapshot> {
        let target = self.latest_pts - duration_us;
        let start_key = self
            .keyframes
            .iter()
            .rev()
            .find(|&&k| k <= target)
            .or_else(|| self.keyframes.front())
            .copied()?;
        let start_idx = self.packets.iter().position(|p| p.is_video_key() && p.pts == start_key)?;
        let packets: Vec<Arc<Packet>> = self
            .packets
            .iter()
            .skip(start_idx)
            .filter(|p| p.stream == VIDEO_STREAM || p.pts >= start_key)
            .cloned()
            .collect();
        Some(Snapshot { packets, start_pts: start_key, end_pts: self.latest_pts })
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Abgedeckte Zeit in µs (für den Füllstand-Ring).
    pub fn buffered_us(&self) -> i64 {
        self.first_pts.map(|f| (self.latest_pts - f).max(0)).unwrap_or(0)
    }

    pub fn pool_len(&self) -> usize {
        self.pools[0].len() + self.pools[1].len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: i64 = 16_667;

    fn limits(secs: i64, bytes: usize) -> Limits {
        Limits { max_duration_us: secs * 1_000_000, max_bytes: bytes }
    }

    /// 60 fps, Keyframe jede Sekunde, Audio alle ~21 ms.
    fn feed(rb: &mut ReplayRingBuffer, secs: i64, size: usize) {
        let data = vec![0u8; size];
        let frames = secs * 60;
        for i in 0..frames {
            let pts = i * FRAME;
            rb.push(VIDEO_STREAM, pts, pts, FRAME, i % 60 == 0, &data);
            if i % 4 == 0 {
                rb.push(1, pts, pts, 21_333, true, &data[..size.min(16)]);
            }
        }
    }

    #[test]
    fn starts_with_keyframe_and_respects_time() {
        let mut rb = ReplayRingBuffer::new(limits(10, usize::MAX));
        feed(&mut rb, 60, 100);
        let first = rb.packets.front().unwrap();
        assert!(first.is_video_key());
        // Zeitfenster 10 s + höchstens eine GOP Überhang.
        assert!(rb.buffered_us() <= 11_000_000, "{}", rb.buffered_us());
        assert!(rb.buffered_us() >= 10_000_000);
    }

    #[test]
    fn respects_byte_limit() {
        let mut rb = ReplayRingBuffer::new(limits(600, 200_000));
        feed(&mut rb, 30, 1000);
        // Höchstens eine GOP (60 * 1000 + Audio) über dem Limit.
        assert!(rb.bytes() <= 200_000 + 61_000, "{}", rb.bytes());
        assert!(rb.packets.front().unwrap().is_video_key());
    }

    #[test]
    fn drops_video_before_first_keyframe() {
        let mut rb = ReplayRingBuffer::new(limits(10, usize::MAX));
        rb.push(VIDEO_STREAM, 0, 0, FRAME, false, &[1]);
        rb.push(1, 0, 0, FRAME, true, &[1]);
        assert!(rb.is_empty());
        rb.push(VIDEO_STREAM, FRAME, FRAME, FRAME, true, &[1]);
        assert_eq!(rb.len(), 1);
    }

    #[test]
    fn snapshot_cuts_at_keyframe_before_window() {
        let mut rb = ReplayRingBuffer::new(limits(60, usize::MAX));
        feed(&mut rb, 40, 10);
        let snap = rb.snapshot(30_000_000).unwrap();
        assert!(snap.packets[0].is_video_key());
        // jetzt = 40 s; Fenster beginnt bei 10 s; Keyframe bei genau 10 s.
        assert_eq!(snap.start_pts, 10 * 60 * FRAME);
        assert!(snap.duration_us() >= 30_000_000);
        assert!(snap.packets.iter().all(|p| p.pts >= snap.start_pts));
    }

    #[test]
    fn snapshot_shorter_than_buffer_returns_all() {
        let mut rb = ReplayRingBuffer::new(limits(60, usize::MAX));
        feed(&mut rb, 5, 10);
        let snap = rb.snapshot(30_000_000).unwrap();
        assert_eq!(snap.start_pts, 0);
        assert_eq!(snap.packets.len(), rb.len());
    }

    #[test]
    fn empty_snapshot_is_none() {
        let rb = ReplayRingBuffer::new(limits(10, usize::MAX));
        assert!(rb.snapshot(1).is_none());
    }

    #[test]
    fn pool_recycles_without_growth() {
        let mut rb = ReplayRingBuffer::new(limits(2, usize::MAX));
        feed(&mut rb, 10, 64);
        let pooled = rb.pool_len();
        assert!(pooled > 0);
        // Nach weiterem Füttern bleibt die Summe aus Puffer und Pool stabil.
        let total = rb.len() + pooled;
        let data = [0u8; 64];
        for i in 0..600 {
            let pts = (600 + i) * FRAME;
            rb.push(VIDEO_STREAM, pts, pts, FRAME, i % 60 == 0, &data);
        }
        assert!(rb.len() + rb.pool_len() <= total + 120);
    }

    #[test]
    fn capacity_does_not_creep() {
        // Große Keyframes, kleine P-Frames, winziges Audio: gezählte Bytes müssen in der
        // Größenordnung der Nutzdaten bleiben, auch nach langer Laufzeit.
        let mut rb = ReplayRingBuffer::new(limits(5, usize::MAX));
        let key = vec![0u8; 300_000];
        let p = vec![0u8; 40_000];
        let a = vec![0u8; 400];
        for i in 0..60 * 120 {
            let pts = i * FRAME;
            let k = i % 60 == 0;
            rb.push(VIDEO_STREAM, pts, pts, FRAME, k, if k { &key } else { &p });
            rb.push(1, pts, pts, FRAME, true, &a);
        }
        let payload: usize = rb.packets.iter().map(|p| p.data.len()).sum();
        assert!(rb.bytes() < payload * 2, "bytes {} vs payload {}", rb.bytes(), payload);
    }

    #[test]
    fn held_snapshot_packets_are_not_recycled() {
        let mut rb = ReplayRingBuffer::new(limits(1, usize::MAX));
        feed(&mut rb, 2, 8);
        let snap = rb.snapshot(1_000_000).unwrap();
        let before: Vec<i64> = snap.packets.iter().map(|p| p.pts).collect();
        feed(&mut rb, 5, 8);
        let after: Vec<i64> = snap.packets.iter().map(|p| p.pts).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn shrinking_limits_evicts() {
        let mut rb = ReplayRingBuffer::new(limits(60, usize::MAX));
        feed(&mut rb, 30, 10);
        rb.set_limits(limits(5, usize::MAX));
        assert!(rb.buffered_us() <= 6_000_000);
    }
}
