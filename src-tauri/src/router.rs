//! Verteilt encodierte Pakete an den Replay-Buffer und eine laufende Aufnahme.
//! Wird von Video-Encoder- und Audio-Engine-Thread geteilt; der Lock ist sehr kurz
//! (Kopie in ein Pool-Paket) und liegt nicht zwischen Capture und Encoder.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crossbeam_channel::Sender;
use parking_lot::{Mutex, RwLock};

use crate::buffer::{Limits, Packet, ReplayRingBuffer, Snapshot};
use crate::stats::Stats;

pub struct PacketRouter {
    ring: Mutex<ReplayRingBuffer>,
    recorder: RwLock<Option<Sender<Arc<Packet>>>>,
    stats: Arc<Stats>,
}

impl PacketRouter {
    pub fn new(limits: Limits, stats: Arc<Stats>) -> Self {
        PacketRouter { ring: Mutex::new(ReplayRingBuffer::new(limits)), recorder: RwLock::new(None), stats }
    }

    pub fn route(&self, stream: u8, pts: i64, dts: i64, dur: i64, key: bool, data: &[u8]) {
        let rec = self.recorder.read();
        let shared = {
            let mut ring = self.ring.lock();
            let p = ring.push_shared(stream, pts, dts, dur, key, data);
            self.stats.buffer_bytes.store(ring.bytes() as u64, Ordering::Relaxed);
            self.stats.buffered_us.store(ring.buffered_us(), Ordering::Relaxed);
            p
        };
        if stream == 0 {
            self.stats.video_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
            self.stats.encoded_frames.fetch_add(1, Ordering::Relaxed);
        }
        if let (Some(tx), Some(p)) = (rec.as_ref(), shared) {
            if tx.try_send(p).is_err() {
                self.stats.recorder_dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn snapshot(&self, duration_us: i64) -> Option<Snapshot> {
        self.ring.lock().snapshot(duration_us)
    }

    pub fn set_limits(&self, limits: Limits) {
        self.ring.lock().set_limits(limits);
    }

    pub fn clear(&self) {
        let mut ring = self.ring.lock();
        ring.clear();
        self.stats.buffer_bytes.store(0, Ordering::Relaxed);
        self.stats.buffered_us.store(0, Ordering::Relaxed);
    }

    pub fn attach_recorder(&self, tx: Option<Sender<Arc<Packet>>>) {
        *self.recorder.write() = tx;
    }

    pub fn is_recording(&self) -> bool {
        self.recorder.read().is_some()
    }
}
