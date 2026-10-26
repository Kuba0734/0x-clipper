//! Erkennung verfügbarer Encoder. Jeder Kandidat wird testweise auf dem echten
//! D3D11-Gerät geöffnet. Das Ergebnis wird pro GPU gecacht, damit der Kaltstart schnell bleibt.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ffmpeg::{EncoderKind, VideoEncoder, VideoEncoderConfig};
use crate::capture::d3d::D3dDevice;
use crate::paths;
use crate::settings::{Codec, QualityPreset, RateControl};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EncoderInfo {
    /// FFmpeg-Name, z. B. `hevc_nvenc`.
    pub id: String,
    pub codec: Codec,
    /// `nvidia`, `amd`, `intel`, `software`.
    pub vendor: String,
    pub hardware: bool,
}

/// Reihenfolge = Priorität bei „automatisch“.
const CANDIDATES: &[(&str, Codec, &str)] = &[
    ("h264_nvenc", Codec::H264, "nvidia"),
    ("hevc_nvenc", Codec::Hevc, "nvidia"),
    ("av1_nvenc", Codec::Av1, "nvidia"),
    ("h264_amf", Codec::H264, "amd"),
    ("hevc_amf", Codec::Hevc, "amd"),
    ("av1_amf", Codec::Av1, "amd"),
    ("h264_qsv", Codec::H264, "intel"),
    ("hevc_qsv", Codec::Hevc, "intel"),
    ("av1_qsv", Codec::Av1, "intel"),
    ("h264_mf", Codec::H264, "software"),
    ("hevc_mf", Codec::Hevc, "software"),
];

#[derive(Debug, Serialize, Deserialize)]
struct Cache {
    key: String,
    encoders: Vec<EncoderInfo>,
}

fn cache_key(adapter_name: &str, adapter_luid: i64) -> String {
    // SAFETY: avcodec_version hat keine Vorbedingungen.
    let ver = unsafe { ffmpeg_sys_next::avcodec_version() };
    format!("{adapter_name}|{adapter_luid:x}|{ver}")
}

/// Gecachtes Ergebnis für den leistungsstärksten Adapter (ohne D3D11-Gerät), falls vorhanden.
pub fn cached() -> Option<Vec<EncoderInfo>> {
    let (name, luid) = D3dDevice::high_performance_adapter().ok()?;
    let path = paths::local_dir().ok()?.join("encoders.json");
    let text = std::fs::read_to_string(path).ok()?;
    let cache: Cache = serde_json::from_str(&text).ok()?;
    (cache.key == cache_key(&name, luid) && !cache.encoders.is_empty()).then_some(cache.encoders)
}

/// Testet alle Kandidaten (parallel, da jeder Open 50–200 ms kosten kann).
pub fn probe(dev: &D3dDevice) -> Vec<EncoderInfo> {
    let results: Vec<Option<EncoderInfo>> = std::thread::scope(|s| {
        let handles: Vec<_> = CANDIDATES
            .iter()
            .map(|&(id, codec, vendor)| {
                s.spawn(move || {
                    let cfg = VideoEncoderConfig {
                        encoder: id.into(),
                        width: 1280,
                        height: 720,
                        fps: 60,
                        rate_control: RateControl::Cbr,
                        bitrate_kbps: 6000,
                        cqp: 23,
                        keyint_secs: 1,
                        preset: QualityPreset::Performance,
                    };
                    match VideoEncoder::new(dev, &cfg) {
                        Ok(_) => Some(EncoderInfo {
                            id: id.into(),
                            codec,
                            vendor: vendor.into(),
                            hardware: EncoderKind::of(id).is_hardware(),
                        }),
                        Err(e) => {
                            tracing::debug!("Encoder {id} nicht verfügbar: {e}");
                            None
                        }
                    }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().ok().flatten()).collect()
    });
    let encoders: Vec<EncoderInfo> = results.into_iter().flatten().collect();
    // Leere Ergebnisse (z. B. Treiberproblem) nie cachen, beim nächsten Start neu testen.
    if encoders.is_empty() {
        tracing::warn!("Kein Encoder nutzbar");
        return encoders;
    }
    if let Ok(dir) = paths::local_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let cache = Cache { key: cache_key(&dev.adapter_name, dev.adapter_luid), encoders: encoders.clone() };
        if let Ok(json) = serde_json::to_vec_pretty(&cache) {
            let _ = std::fs::write(dir.join("encoders.json"), json);
        }
    }
    tracing::info!("Encoder erkannt: {:?}", encoders.iter().map(|e| &e.id).collect::<Vec<_>>());
    encoders
}

/// Wählt den Encoder: explizite Wahl, sonst bester Hardware-Encoder für den Codec,
/// sonst H.264 per Media Foundation.
pub fn choose(available: &[EncoderInfo], wanted: &str, codec: Codec) -> Option<EncoderInfo> {
    if wanted != "auto" {
        if let Some(e) = available.iter().find(|e| e.id == wanted) {
            return Some(e.clone());
        }
    }
    available
        .iter()
        .find(|e| e.codec == codec && e.hardware)
        .or_else(|| available.iter().find(|e| e.codec == codec))
        .or_else(|| available.iter().find(|e| e.codec == Codec::H264))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(id: &str, codec: Codec, hw: bool) -> EncoderInfo {
        EncoderInfo { id: id.into(), codec, vendor: "x".into(), hardware: hw }
    }

    #[test]
    fn auto_prefers_hardware_for_codec() {
        let list = vec![
            enc("h264_nvenc", Codec::H264, true),
            enc("hevc_nvenc", Codec::Hevc, true),
            enc("h264_mf", Codec::H264, false),
        ];
        assert_eq!(choose(&list, "auto", Codec::Hevc).unwrap().id, "hevc_nvenc");
        // AV1 nicht verfügbar → H.264.
        assert_eq!(choose(&list, "auto", Codec::Av1).unwrap().id, "h264_nvenc");
        assert_eq!(choose(&list, "h264_mf", Codec::H264).unwrap().id, "h264_mf");
        // Unbekannte explizite Wahl → automatisch.
        assert_eq!(choose(&list, "av1_qsv", Codec::H264).unwrap().id, "h264_nvenc");
    }

    #[test]
    fn software_only() {
        let list = vec![enc("h264_mf", Codec::H264, false)];
        assert_eq!(choose(&list, "auto", Codec::Hevc).unwrap().id, "h264_mf");
        assert!(choose(&[], "auto", Codec::H264).is_none());
    }
}
