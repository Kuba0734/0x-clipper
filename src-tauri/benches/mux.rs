//! Benchmark „Snapshot → MP4“ (Remux ohne Re-Encode) mit echten H.264/AAC-Parametern.
//! Nutzt h264_mf (immer vorhanden), damit der Bench ohne bestimmte GPU läuft.
use criterion::{criterion_group, criterion_main, Criterion};

use clipper_lib::audio::aac::AacEncoder;
use clipper_lib::buffer::{Limits, ReplayRingBuffer, VIDEO_STREAM};
use clipper_lib::capture::d3d::D3dDevice;
use clipper_lib::encode::ffmpeg::{VideoEncoder, VideoEncoderConfig};
use clipper_lib::encode::StreamSet;
use clipper_lib::mux::mp4::write_snapshot;
use clipper_lib::settings::{QualityPreset, RateControl};

fn bench(c: &mut Criterion) {
    let Ok(dev) = D3dDevice::high_performance() else { return eprintln!("kein D3D11") };
    let cfg = VideoEncoderConfig {
        encoder: "h264_mf".into(),
        width: 1920,
        height: 1080,
        fps: 60,
        rate_control: RateControl::Cbr,
        bitrate_kbps: 20_000,
        cqp: 23,
        keyint_secs: 1,
        preset: QualityPreset::Balanced,
    };
    let (Ok(enc), Ok(aac)) = (VideoEncoder::new(&dev, &cfg), AacEncoder::new(160)) else {
        return eprintln!("Encoder nicht verfügbar");
    };
    let (Ok(video), Ok(audio)) = (enc.params(), aac.params()) else { return };
    let streams = StreamSet { video, audio: vec![audio], fps: 60 };

    // 30 s synthetische Pakete (~20 Mbit/s). Der Muxer prüft den Bitstream nicht.
    let mut rb = ReplayRingBuffer::new(Limits { max_duration_us: 30_000_000, max_bytes: 1 << 30 });
    let data = vec![0u8; 41_000];
    for i in 0..1800i64 {
        let pts = i * 16_667;
        rb.push(VIDEO_STREAM, pts, pts, 16_667, i % 60 == 0, &data);
        if i % 3 == 0 {
            rb.push(1, pts, pts, 21_333, true, &data[..400]);
        }
    }
    let Some(snap) = rb.snapshot(30_000_000) else { return };
    let path = std::env::temp_dir().join("0x-clipper-bench.mp4");
    let mut g = c.benchmark_group("mux");
    g.sample_size(20);
    g.bench_function("write_snapshot_30s_75MB", |b| {
        b.iter(|| write_snapshot(&path, &streams, &snap).map(|_| ()).unwrap_or(()));
    });
    g.finish();
    let _ = std::fs::remove_file(path);
}

criterion_group!(benches, bench);
criterion_main!(benches);
