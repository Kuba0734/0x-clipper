//! WASAPI-Capture (System-Loopback und Mikrofon), event-gesteuert.
//! Liefert 48 kHz Stereo f32 (interleaved) mit QPC-Zeitstempeln.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;
use ts_rs::TS;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE};
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW,
    WaitForSingleObject,
};

use crate::error::{CoreError, CoreResult};
use crate::util;

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AudioDevices {
    pub outputs: Vec<AudioDevice>,
    pub inputs: Vec<AudioDevice>,
}

fn enumerator() -> CoreResult<IMMDeviceEnumerator> {
    util::com_init();
    // SAFETY: Standard-COM-Instanziierung; Ergebnis wird geprüft.
    Ok(unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? })
}

pub fn list_devices() -> CoreResult<AudioDevices> {
    let e = enumerator()?;
    Ok(AudioDevices { outputs: list_flow(&e, eRender)?, inputs: list_flow(&e, eCapture)? })
}

fn device_id(d: &IMMDevice) -> CoreResult<String> {
    // SAFETY: GetId liefert einen CoTaskMem-String, den wir nach dem Kopieren freigeben.
    unsafe {
        let p = d.GetId()?;
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const _));
        Ok(s)
    }
}

fn list_flow(e: &IMMDeviceEnumerator, flow: EDataFlow) -> CoreResult<Vec<AudioDevice>> {
    // SAFETY: Reine COM-Abfragen; alle Ergebnisse geprüft.
    unsafe {
        let default_id = e
            .GetDefaultAudioEndpoint(flow, eConsole)
            .ok()
            .and_then(|d| device_id(&d).ok())
            .unwrap_or_default();
        let coll = e.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
        let mut out = Vec::new();
        for i in 0..coll.GetCount()? {
            let d = coll.Item(i)?;
            let id = device_id(&d)?;
            let name = d
                .OpenPropertyStore(STGM_READ)
                .and_then(|s| s.GetValue(&PKEY_Device_FriendlyName))
                .map(|v| v.to_string())
                .unwrap_or_else(|_| id.clone());
            out.push(AudioDevice { is_default: id == default_id, id, name });
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// System-Audio über Loopback des Ausgabegeräts.
    Loopback,
    Mic,
}

/// Ein Block Samples einer Quelle. Der Puffer wandert über `free` zurück (keine Allokation).
pub struct AudioChunk {
    pub source: SourceKind,
    /// QPC-Zeit des ersten Samples (µs).
    pub ts_us: i64,
    /// Interleaved Stereo, 48 kHz.
    pub data: Vec<f32>,
}

struct Event(HANDLE);
impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: Eigenes Event-Handle.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Eingangsformat, falls Windows nicht selbst auf 48 kHz Stereo f32 konvertiert.
#[derive(Debug, Clone, Copy)]
enum SampleFormat {
    F32,
    I16,
    I32,
}

/// Minimaler Konverter: beliebige Kanäle → Stereo, beliebige Rate → 48 kHz (linear).
struct Converter {
    format: SampleFormat,
    channels: usize,
    step: f64,
    pos: f64,
    prev: [f32; 2],
}

impl Converter {
    fn identity() -> Self {
        Converter { format: SampleFormat::F32, channels: 2, step: 1.0, pos: 0.0, prev: [0.0; 2] }
    }

    fn is_identity(&self) -> bool {
        matches!(self.format, SampleFormat::F32) && self.channels == 2 && self.step == 1.0
    }

    fn sample(&self, raw: &[u8], frame: usize, ch: usize) -> f32 {
        let ch = ch.min(self.channels - 1);
        let idx = frame * self.channels + ch;
        match self.format {
            SampleFormat::F32 => {
                let b = &raw[idx * 4..idx * 4 + 4];
                f32::from_le_bytes([b[0], b[1], b[2], b[3]])
            }
            SampleFormat::I16 => {
                let b = &raw[idx * 2..idx * 2 + 2];
                i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0
            }
            SampleFormat::I32 => {
                let b = &raw[idx * 4..idx * 4 + 4];
                i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0
            }
        }
    }

    fn convert(&mut self, raw: &[u8], frames: usize, out: &mut Vec<f32>) {
        if self.is_identity() {
            out.extend(raw.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
            return;
        }
        let stereo = |s: &Self, f: usize| {
            if s.channels == 1 {
                let v = s.sample(raw, f, 0);
                [v, v]
            } else {
                [s.sample(raw, f, 0), s.sample(raw, f, 1)]
            }
        };
        while self.pos < frames as f64 {
            let i = self.pos.floor() as usize;
            let t = (self.pos - i as f64) as f32;
            let a = if i == 0 { self.prev } else { stereo(self, i - 1) };
            let b = stereo(self, i);
            out.push(a[0] + (b[0] - a[0]) * t);
            out.push(a[1] + (b[1] - a[1]) * t);
            self.pos += self.step;
        }
        self.pos -= frames as f64;
        if frames > 0 {
            self.prev = stereo(self, frames - 1);
        }
    }
}

fn desired_format() -> WAVEFORMATEXTENSIBLE {
    let block = (CHANNELS * 4) as u16;
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
            nChannels: CHANNELS as u16,
            nSamplesPerSec: SAMPLE_RATE,
            nAvgBytesPerSec: SAMPLE_RATE * block as u32,
            nBlockAlign: block,
            wBitsPerSample: 32,
            cbSize: (std::mem::size_of::<WAVEFORMATEXTENSIBLE>() - std::mem::size_of::<WAVEFORMATEX>()) as u16,
        },
        Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: 32 },
        // SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT
        dwChannelMask: 0x3,
        SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
    }
}

/// Liest das Format aus einer Mix-Format-Struktur.
///
/// # Safety
/// `fmt` muss auf ein gültiges WAVEFORMATEX (ggf. EXTENSIBLE) zeigen.
unsafe fn converter_for(fmt: *const WAVEFORMATEX) -> CoreResult<Converter> {
    // SAFETY: laut Vertrag gültig; EXTENSIBLE wird nur bei passendem Tag gelesen.
    unsafe {
        let f = &*fmt;
        let float = if f.wFormatTag == WAVE_FORMAT_EXTENSIBLE as u16 {
            let ext = &*(fmt as *const WAVEFORMATEXTENSIBLE);
            let sub = ext.SubFormat;
            if sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
                true
            } else if sub == KSDATAFORMAT_SUBTYPE_PCM {
                false
            } else {
                return Err(CoreError::Audio("Unbekanntes Audioformat".into()));
            }
        } else {
            f.wFormatTag == WAVE_FORMAT_IEEE_FLOAT as u16
        };
        let format = match (float, f.wBitsPerSample) {
            (true, 32) => SampleFormat::F32,
            (false, 16) => SampleFormat::I16,
            (false, 32) => SampleFormat::I32,
            (_, bits) => return Err(CoreError::Audio(format!("{bits}-Bit-Audio nicht unterstützt"))),
        };
        Ok(Converter {
            format,
            channels: f.nChannels.max(1) as usize,
            step: f.nSamplesPerSec as f64 / SAMPLE_RATE as f64,
            pos: 0.0,
            prev: [0.0; 2],
        })
    }
}

pub struct CaptureParams {
    pub kind: SourceKind,
    pub device_id: String,
    pub tx: Sender<AudioChunk>,
    pub free_rx: Receiver<Vec<f32>>,
    pub stop: Arc<AtomicBool>,
}

/// Thread-Hauptfunktion einer Audioquelle.
pub fn run_capture(p: CaptureParams) -> CoreResult<()> {
    util::com_init();
    let mut task_index = 0u32;
    // SAFETY: Registriert den Thread bei MMCSS ("Audio"); wird unten wieder zurückgesetzt.
    let mmcss = unsafe { AvSetMmThreadCharacteristicsW(w!("Audio"), &mut task_index) }.ok();
    let r = capture_loop(&p);
    if let Some(h) = mmcss {
        // SAFETY: Handle stammt aus AvSetMmThreadCharacteristicsW.
        unsafe {
            let _ = AvRevertMmThreadCharacteristics(h);
        }
    }
    r
}

fn open_device(e: &IMMDeviceEnumerator, kind: SourceKind, id: &str) -> CoreResult<IMMDevice> {
    let flow = if kind == SourceKind::Loopback { eRender } else { eCapture };
    // SAFETY: COM-Aufrufe mit gültigen Parametern.
    unsafe {
        if !id.is_empty() {
            if let Ok(d) = e.GetDevice(PCWSTR(HSTRING::from(id).as_ptr())) {
                return Ok(d);
            }
            tracing::warn!("Audiogerät {id} nicht gefunden, nutze Standardgerät");
        }
        Ok(e.GetDefaultAudioEndpoint(flow, eConsole)?)
    }
}

/// Spielt Stille auf dem Ausgabegerät, damit Loopback auch ohne Ton durchgehend Pakete liefert.
fn start_silence(device: &IMMDevice) -> CoreResult<IAudioClient> {
    // SAFETY: Standard-WASAPI-Initialisierung im Shared Mode; Mix-Format wird freigegeben.
    unsafe {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let fmt = client.GetMixFormat()?;
        let r = client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 1_000_000, 0, fmt, None);
        CoTaskMemFree(Some(fmt as *const _));
        r?;
        let frames = client.GetBufferSize()?;
        let render: IAudioRenderClient = client.GetService()?;
        let _ = render.GetBuffer(frames)?;
        render.ReleaseBuffer(frames, AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)?;
        client.Start()?;
        Ok(client)
    }
}

fn capture_loop(p: &CaptureParams) -> CoreResult<()> {
    let e = enumerator()?;
    let device = open_device(&e, p.kind, &p.device_id)?;
    let loopback = p.kind == SourceKind::Loopback;
    // SAFETY: WASAPI-Setup; alle Rückgaben geprüft, Speicher von GetMixFormat freigegeben.
    let (client, capture, mut conv, event) = unsafe {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let base = AUDCLNT_STREAMFLAGS_EVENTCALLBACK | if loopback { AUDCLNT_STREAMFLAGS_LOOPBACK } else { 0 };
        let want = desired_format();
        let auto = base | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        let conv = match client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            auto,
            2_000_000,
            0,
            &want.Format,
            None,
        ) {
            Ok(()) => Converter::identity(),
            Err(err) => {
                tracing::debug!("AUTOCONVERTPCM abgelehnt ({err}), nutze Mix-Format");
                let client2: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
                let fmt = client2.GetMixFormat()?;
                let conv = converter_for(fmt);
                let r = client2.Initialize(AUDCLNT_SHAREMODE_SHARED, base, 2_000_000, 0, fmt, None);
                CoTaskMemFree(Some(fmt as *const _));
                r?;
                let conv = conv?;
                return finish_loop(p, &device, client2, conv, loopback);
            }
        };
        let event = Event(CreateEventW(None, false, false, None)?);
        client.SetEventHandle(event.0)?;
        let capture: IAudioCaptureClient = client.GetService()?;
        (client, capture, conv, event)
    };
    run_stream(p, &device, &client, &capture, &mut conv, &event, loopback)
}

fn finish_loop(
    p: &CaptureParams,
    device: &IMMDevice,
    client: IAudioClient,
    mut conv: Converter,
    loopback: bool,
) -> CoreResult<()> {
    // SAFETY: Client ist initialisiert.
    let (capture, event) = unsafe {
        let event = Event(CreateEventW(None, false, false, None)?);
        client.SetEventHandle(event.0)?;
        let capture: IAudioCaptureClient = client.GetService()?;
        (capture, event)
    };
    run_stream(p, device, &client, &capture, &mut conv, &event, loopback)
}

fn run_stream(
    p: &CaptureParams,
    device: &IMMDevice,
    client: &IAudioClient,
    capture: &IAudioCaptureClient,
    conv: &mut Converter,
    event: &Event,
    loopback: bool,
) -> CoreResult<()> {
    let _silence = if loopback { start_silence(device).ok() } else { None };
    let block_bytes = if conv.is_identity() { CHANNELS * 4 } else {
        conv.channels * match conv.format { SampleFormat::I16 => 2, _ => 4 }
    };
    // SAFETY: Client ist initialisiert.
    unsafe { client.Start()? };
    while !p.stop.load(Ordering::Relaxed) {
        // SAFETY: Eigenes Event-Handle; Timeout, damit Stop zeitnah greift.
        let w = unsafe { WaitForSingleObject(event.0, 200) };
        if w != WAIT_OBJECT_0 {
            continue;
        }
        loop {
            // SAFETY: Sequenz GetNextPacketSize → GetBuffer → ReleaseBuffer wie dokumentiert.
            unsafe {
                if capture.GetNextPacketSize()? == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                let mut qpc = 0u64;
                capture.GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))?;
                let mut buf = p.free_rx.try_recv().unwrap_or_else(|_| Vec::with_capacity(4096));
                buf.clear();
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                    let out_frames = (frames as f64 / conv.step).round() as usize;
                    buf.resize(out_frames * CHANNELS, 0.0);
                } else {
                    let raw = std::slice::from_raw_parts(data, frames as usize * block_bytes);
                    conv.convert(raw, frames as usize, &mut buf);
                }
                capture.ReleaseBuffer(frames)?;
                // u64QPCPosition ist in 100-ns-Einheiten.
                let chunk = AudioChunk { source: p.kind, ts_us: (qpc / 10) as i64, data: buf };
                if let Err(e) = p.tx.try_send(chunk) {
                    tracing::debug!("Audio-Chunk verworfen ({:?})", p.kind);
                    drop(e);
                }
            }
        }
    }
    // SAFETY: Client läuft.
    unsafe {
        let _ = client.Stop();
    }
    Ok(())
}
