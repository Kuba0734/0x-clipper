//! DXGI Desktop Duplication als Fallback (ältere Systeme, WGC nicht verfügbar).
//! `AcquireNextFrame` blockiert bis zum nächsten Frame oder Timeout – kein Polling.
//! Einschränkung: Nur ganze Monitore, Mauszeiger wird nicht eingezeichnet.

use std::time::Duration;

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::{
    IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource, DXGI_ERROR_ACCESS_LOST,
    DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};

use super::d3d::D3dDevice;
use super::{FrameSource, SourceFrame};
use crate::error::{CoreError, CoreResult};
use crate::util;

pub struct DxgiSource {
    device: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    output: IDXGIOutput1,
    dup: IDXGIOutputDuplication,
    holding: bool,
    size: (u32, u32),
}

// SAFETY: Die Duplikation wird ausschließlich vom Capture-Thread benutzt.
unsafe impl Send for DxgiSource {}

impl DxgiSource {
    pub fn new(dev: &D3dDevice, output: &IDXGIOutput) -> CoreResult<Self> {
        let output: IDXGIOutput1 = output.cast()?;
        // SAFETY: Gerät und Output gehören zum selben Adapter (D3dDevice::for_monitor).
        let dup = unsafe { output.DuplicateOutput(&dev.device)? };
        // SAFETY: Reiner Getter.
        let desc = unsafe { dup.GetDesc() };
        Ok(DxgiSource {
            device: dev.device.clone(),
            output,
            dup,
            holding: false,
            size: (desc.ModeDesc.Width, desc.ModeDesc.Height),
        })
    }

    fn recreate(&mut self) -> CoreResult<()> {
        // SAFETY: wie in new().
        self.dup = unsafe { self.output.DuplicateOutput(&self.device)? };
        Ok(())
    }
}

impl FrameSource for DxgiSource {
    fn next_frame(&mut self, timeout: Duration) -> CoreResult<Option<SourceFrame>> {
        self.release();
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        // SAFETY: Ausgabezeiger sind gültig; vorheriger Frame wurde freigegeben.
        let r = unsafe { self.dup.AcquireNextFrame(timeout.as_millis() as u32, &mut info, &mut resource) };
        match r {
            Ok(()) => {}
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(None),
            Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                // Moduswechsel, UAC, Vollbild-Wechsel: Duplikation neu anlegen.
                self.recreate()?;
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        }
        self.holding = true;
        // Nur Mausbewegung, kein neues Bild.
        if info.LastPresentTime == 0 {
            self.release();
            return Ok(None);
        }
        let texture: ID3D11Texture2D = resource
            .ok_or_else(|| CoreError::Capture("Kein Desktop-Bild".into()))?
            .cast()?;
        Ok(Some(SourceFrame { texture, pts_us: util::qpc_to_us(info.LastPresentTime) }))
    }

    fn release(&mut self) {
        if self.holding {
            // SAFETY: Wir halten genau einen Frame.
            unsafe {
                let _ = self.dup.ReleaseFrame();
            }
            self.holding = false;
        }
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }
}

impl Drop for DxgiSource {
    fn drop(&mut self) {
        self.release();
    }
}
