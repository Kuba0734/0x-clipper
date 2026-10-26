//! Benchmarks des Ringpuffers: Push im eingeschwungenen Zustand (heißer Pfad) und Snapshot.
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

use clipper_lib::buffer::{Limits, ReplayRingBuffer, VIDEO_STREAM};

const FRAME: i64 = 16_667;
/// ~20 Mbit/s bei 60 fps.
const PKT: usize = 41_000;

fn filled(secs: i64) -> (ReplayRingBuffer, i64) {
    let mut rb = ReplayRingBuffer::new(Limits { max_duration_us: 30_000_000, max_bytes: 512 << 20 });
    let data = vec![0u8; PKT];
    let frames = secs * 60;
    for i in 0..frames {
        let pts = i * FRAME;
        rb.push(VIDEO_STREAM, pts, pts, FRAME, i % 60 == 0, &data);
        if i % 3 == 0 {
            rb.push(1, pts, pts, 21_333, true, &data[..400]);
        }
    }
    (rb, frames)
}

fn bench(c: &mut Criterion) {
    let mut g = c.benchmark_group("ring");
    g.throughput(Throughput::Bytes(PKT as u64));
    g.bench_function("push_video_packet_steady_state", |b| {
        let (mut rb, mut i) = filled(40);
        let data = vec![0u8; PKT];
        b.iter(|| {
            let pts = i * FRAME;
            rb.push(VIDEO_STREAM, pts, pts, FRAME, i % 60 == 0, black_box(&data));
            i += 1;
        });
    });
    g.finish();

    c.bench_function("snapshot_30s", |b| {
        let (rb, _) = filled(40);
        b.iter(|| black_box(rb.snapshot(30_000_000)));
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
