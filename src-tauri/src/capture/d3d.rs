//! D3D11-Gerät und GPU-Konvertierung BGRA → NV12 inklusive Skalierung
//! über den D3D11 Video Processor (ein Durchgang, kein CPU-Readback).

use windows::core::Interface;
use windows::Win32::Foundation::{HMODULE, RECT};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_RATIONAL};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter, IDXGIAdapter1, IDXGIFactory1, IDXGIFactory6,
    DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
};
use windows::Win32::Graphics::Gdi::HMONITOR;

use crate::error::{CoreError, CoreResult};

pub struct D3dDevice {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub adapter_name: String,
    pub adapter_luid: i64,
}

// SAFETY: Das Gerät ist mit SetMultithreadProtected(TRUE) erstellt; der Immediate Context
// wird zusätzlich nur unter FFmpegs D3D11-Lock bzw. aus genau einem Thread benutzt.
unsafe impl Send for D3dDevice {}
// SAFETY: siehe oben.
unsafe impl Sync for D3dDevice {}

impl D3dDevice {
    /// Gerät auf dem leistungsstärksten Adapter (bei Hybrid-GPUs die dGPU mit NVENC/AMF).
    pub fn high_performance() -> CoreResult<Self> {
        // SAFETY: COM-Fabrik-Aufrufe ohne Zeiger-Vorbedingungen.
        let adapter: Option<IDXGIAdapter1> = unsafe {
            let f: IDXGIFactory1 = CreateDXGIFactory1()?;
            match f.cast::<IDXGIFactory6>() {
                Ok(f6) => f6.EnumAdapterByGpuPreference(0, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE).ok(),
                Err(_) => f.EnumAdapters1(0).ok(),
            }
        };
        Self::create(adapter)
    }

    /// Gerät auf dem Adapter, an dem der Monitor hängt (nötig für DXGI Desktop Duplication).
    pub fn for_monitor(monitor: HMONITOR) -> CoreResult<(Self, windows::Win32::Graphics::Dxgi::IDXGIOutput)> {
        // SAFETY: Reine COM-Enumeration; alle Rückgaben werden geprüft.
        unsafe {
            let f: IDXGIFactory1 = CreateDXGIFactory1()?;
            let mut i = 0;
            while let Ok(adapter) = f.EnumAdapters1(i) {
                let mut j = 0;
                while let Ok(output) = adapter.EnumOutputs(j) {
                    if output.GetDesc()?.Monitor == monitor {
                        return Ok((Self::create(Some(adapter))?, output));
                    }
                    j += 1;
                }
                i += 1;
            }
        }
        Err(CoreError::Capture("Monitor an keinem Adapter gefunden".into()))
    }

    fn create(adapter: Option<IDXGIAdapter1>) -> CoreResult<Self> {
        let mut device = None;
        let mut context = None;
        let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT;
        let (name, luid) = adapter.as_ref().map(adapter_info).unwrap_or_default();
        let base: Option<IDXGIAdapter> = adapter.as_ref().and_then(|a| a.cast().ok());
        // SAFETY: Ausgabezeiger sind gültige Option-Slots; Feature-Level-Slice lebt im Aufruf.
        unsafe {
            D3D11CreateDevice(
                base.as_ref(),
                if base.is_some() { D3D_DRIVER_TYPE_UNKNOWN } else { windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE },
                HMODULE::default(),
                flags,
                // 11_1 wird für NV12 als Render-Target (Video-Processor-Ausgabe) gebraucht.
                Some(&[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }
        let device: ID3D11Device =
            device.ok_or_else(|| CoreError::Capture("D3D11CreateDevice ohne Gerät".into()))?;
        let context = context.ok_or_else(|| CoreError::Capture("Kein Device Context".into()))?;
        if let Ok(mt) = device.cast::<ID3D11Multithread>() {
            // SAFETY: Einfacher Flag-Setter auf gültigem Interface.
            unsafe {
                let _ = mt.SetMultithreadProtected(true);
            }
        }
        Ok(D3dDevice { device, context, adapter_name: name, adapter_luid: luid })
    }
}

fn adapter_info(a: &IDXGIAdapter1) -> (String, i64) {
    // SAFETY: Reiner Getter.
    let Ok(desc) = (unsafe { a.GetDesc1() }) else {
        return (String::new(), 0);
    };
    let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
    let name = String::from_utf16_lossy(&desc.Description[..len]);
    let luid = ((desc.AdapterLuid.HighPart as i64) << 32) | desc.AdapterLuid.LowPart as i64;
    (name, luid)
}

/// BGRA-Textur → NV12-Slice eines FFmpeg-Texture-Arrays, inklusive Skalierung/Letterbox.
pub struct VideoConverter {
    video_device: ID3D11VideoDevice,
    video_context: ID3D11VideoContext,
    enumerator: ID3D11VideoProcessorEnumerator,
    processor: ID3D11VideoProcessor,
    /// Eingabe-Views je Quelltextur (WGC rotiert durch wenige Texturen).
    inputs: Vec<(usize, ID3D11VideoProcessorInputView)>,
    /// Ausgabe-Views je Array-Slice des FFmpeg-Pools.
    outputs: Vec<(usize, u32, ID3D11VideoProcessorOutputView)>,
    staging: Option<ID3D11Texture2D>,
    pub in_size: (u32, u32),
    pub out_size: (u32, u32),
}

impl VideoConverter {
    pub fn new(dev: &D3dDevice, in_size: (u32, u32), out_size: (u32, u32), fps: u32) -> CoreResult<Self> {
        let video_device: ID3D11VideoDevice = dev.device.cast()?;
        let video_context: ID3D11VideoContext = dev.context.cast()?;
        let rate = DXGI_RATIONAL { Numerator: fps, Denominator: 1 };
        let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: rate,
            InputWidth: in_size.0,
            InputHeight: in_size.1,
            OutputFrameRate: rate,
            OutputWidth: out_size.0,
            OutputHeight: out_size.1,
            Usage: D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
        };
        // SAFETY: desc lebt über den Aufruf; Rückgaben geprüft.
        let (enumerator, processor) = unsafe {
            let e = video_device.CreateVideoProcessorEnumerator(&desc)?;
            let p = video_device.CreateVideoProcessor(&e, 0)?;
            (e, p)
        };
        let me = VideoConverter {
            video_device,
            video_context,
            enumerator,
            processor,
            inputs: Vec::with_capacity(4),
            outputs: Vec::with_capacity(32),
            staging: None,
            in_size,
            out_size,
        };
        me.configure();
        Ok(me)
    }

    fn configure(&self) {
        let (iw, ih) = self.in_size;
        let (ow, oh) = self.out_size;
        // Seitenverhältnis erhalten (Letterbox/Pillarbox).
        let scale = (ow as f64 / iw.max(1) as f64).min(oh as f64 / ih.max(1) as f64);
        let dw = ((iw as f64 * scale).round() as i32).min(ow as i32);
        let dh = ((ih as f64 * scale).round() as i32).min(oh as i32);
        let dx = (ow as i32 - dw) / 2;
        let dy = (oh as i32 - dh) / 2;
        let src = RECT { left: 0, top: 0, right: iw as i32, bottom: ih as i32 };
        let dst = RECT { left: dx, top: dy, right: dx + dw, bottom: dy + dh };
        let full = RECT { left: 0, top: 0, right: ow as i32, bottom: oh as i32 };
        // Bitfeld: Usage(1) RGB_Range(1) YCbCr_Matrix(1) xvYCC(1) Nominal_Range(2).
        // Eingang: RGB Vollbereich (0). Ausgang: BT.709, Studio-Bereich 16–235.
        let input_cs = D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: 0 };
        let output_cs = D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: (1 << 2) | (1 << 4) };
        let black = D3D11_VIDEO_COLOR {
            Anonymous: D3D11_VIDEO_COLOR_0 {
                YCbCr: D3D11_VIDEO_COLOR_YCbCrA { Y: 0.0625, Cb: 0.5, Cr: 0.5, A: 1.0 },
            },
        };
        let vc = &self.video_context;
        let p = &self.processor;
        // SAFETY: Alle Strukturen leben über die Aufrufe; Processor gehört zu diesem Context.
        unsafe {
            vc.VideoProcessorSetStreamFrameFormat(p, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
            vc.VideoProcessorSetStreamSourceRect(p, 0, true, Some(&src));
            vc.VideoProcessorSetStreamDestRect(p, 0, true, Some(&dst));
            vc.VideoProcessorSetOutputTargetRect(p, true, Some(&full));
            vc.VideoProcessorSetStreamColorSpace(p, 0, &input_cs);
            vc.VideoProcessorSetOutputColorSpace(p, &output_cs);
            vc.VideoProcessorSetOutputBackgroundColor(p, true, &black);
            vc.VideoProcessorSetStreamAutoProcessingMode(p, 0, false);
        }
    }

    fn input_view(&mut self, tex: &ID3D11Texture2D) -> CoreResult<ID3D11VideoProcessorInputView> {
        let key = tex.as_raw() as usize;
        if let Some((_, v)) = self.inputs.iter().find(|(k, _)| *k == key) {
            return Ok(v.clone());
        }
        let desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV { MipSlice: 0, ArraySlice: 0 },
            },
        };
        let mut view = None;
        // SAFETY: Textur und Enumerator sind gültig; view ist ein Ausgabeslot.
        unsafe {
            self.video_device.CreateVideoProcessorInputView(tex, &self.enumerator, &desc, Some(&mut view))?;
        }
        let view = view.ok_or_else(|| CoreError::Capture("Kein Input-View".into()))?;
        if self.inputs.len() >= 8 {
            self.inputs.remove(0);
        }
        self.inputs.push((key, view.clone()));
        Ok(view)
    }

    fn output_view(&mut self, tex: &ID3D11Texture2D, slice: u32) -> CoreResult<ID3D11VideoProcessorOutputView> {
        let key = tex.as_raw() as usize;
        if let Some((_, _, v)) = self.outputs.iter().find(|(k, s, _)| *k == key && *s == slice) {
            return Ok(v.clone());
        }
        let desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
            ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2DARRAY,
            Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                Texture2DArray: D3D11_TEX2D_ARRAY_VPOV { MipSlice: 0, FirstArraySlice: slice, ArraySize: 1 },
            },
        };
        let mut view = None;
        // SAFETY: wie input_view.
        unsafe {
            self.video_device.CreateVideoProcessorOutputView(tex, &self.enumerator, &desc, Some(&mut view))?;
        }
        let view = view.ok_or_else(|| CoreError::Capture("Kein Output-View".into()))?;
        self.outputs.push((key, slice, view.clone()));
        Ok(view)
    }

    /// Wie `blt`, aber über eine eigene NV12-Zwischentextur und eine GPU-Kopie in `dst[slice]`
    /// (für Texture-Arrays, die kein Render-Target sein dürfen, z. B. QSV).
    pub fn blt_copy(
        &mut self,
        dev: &D3dDevice,
        src: &ID3D11Texture2D,
        dst: &ID3D11Texture2D,
        slice: u32,
    ) -> CoreResult<()> {
        if self.staging.is_none() {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: self.out_size.0,
                Height: self.out_size.1,
                MipLevels: 1,
                ArraySize: 1,
                Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_NV12,
                SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut t = None;
            // SAFETY: desc lebt über den Aufruf; t ist ein Ausgabeslot.
            unsafe { dev.device.CreateTexture2D(&desc, None, Some(&mut t))? };
            self.staging = t;
        }
        let staging = self.staging.clone().ok_or(CoreError::NotRunning)?;
        // Das Zwischen-Render-Target ist kein Array: Slice 0 über einen Array-View geht trotzdem.
        self.blt(src, &staging, 0)?;
        // SAFETY: Gleiches Format und gleiche Größe; Subresource-Index = Slice (MipLevels = 1).
        unsafe { dev.context.CopySubresourceRegion(dst, slice, 0, 0, 0, &staging, 0, None) };
        Ok(())
    }

    /// Schreibt `src` skaliert als NV12 in `dst[slice]`. Läuft vollständig auf der GPU.
    pub fn blt(&mut self, src: &ID3D11Texture2D, dst: &ID3D11Texture2D, slice: u32) -> CoreResult<()> {
        let input = self.input_view(src)?;
        let output = self.output_view(dst, slice)?;
        let stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: std::mem::ManuallyDrop::new(Some(input)),
            ..Default::default()
        };
        let streams = [stream];
        // SAFETY: Views gehören zu diesem Processor; streams lebt über den Aufruf.
        let r = unsafe { self.video_context.VideoProcessorBlt(&self.processor, &output, 0, &streams) };
        // pInputSurface ist ManuallyDrop: Referenz hier explizit freigeben.
        let [mut s] = streams;
        // SAFETY: Wir geben genau die eine Referenz frei, die wir oben hineingelegt haben.
        unsafe { std::mem::ManuallyDrop::drop(&mut s.pInputSurface) };
        r.map_err(CoreError::from)
    }
}

/// Liest die Abmessungen einer Textur.
pub fn texture_size(tex: &ID3D11Texture2D) -> (u32, u32) {
    let mut d = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: d ist ein gültiger Ausgabepuffer.
    unsafe { tex.GetDesc(&mut d) };
    (d.Width, d.Height)
}

/// Prüft, ob eine Textur BGRA ist (WGC liefert immer BGRA, DXGI meist auch).
pub fn is_bgra(tex: &ID3D11Texture2D) -> bool {
    let mut d = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: wie oben.
    unsafe { tex.GetDesc(&mut d) };
    d.Format == DXGI_FORMAT_B8G8R8A8_UNORM
}
