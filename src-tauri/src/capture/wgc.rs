//! Windows Graphics Capture. Der FrameArrived-Handler weckt nur den Capture-Thread
//! (bounded(1), keine Allokation); Frames werden dort per TryGetNextFrame abgeholt.

use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, RecvTimeoutError};
use windows::core::{IInspectable, Interface};
use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

use super::d3d::D3dDevice;
use super::{FrameSource, SourceFrame};
use crate::error::{CoreError, CoreResult};

pub enum WgcTarget {
    Monitor(HMONITOR),
    Window(HWND),
}

pub struct WgcSource {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    _item: GraphicsCaptureItem,
    device: IDirect3DDevice,
    wake: Receiver<()>,
    closed: Receiver<()>,
    current: Option<Direct3D11CaptureFrame>,
    size: SizeInt32,
    arrived_token: i64,
}

// SAFETY: Free-threaded Frame-Pool und Session sind agil (beliebige Threads erlaubt).
unsafe impl Send for WgcSource {}

pub fn is_supported() -> bool {
    GraphicsCaptureSession::IsSupported().unwrap_or(false)
}

impl WgcSource {
    pub fn new(dev: &D3dDevice, target: WgcTarget, cursor: bool, hide_border: bool, fps: u32) -> CoreResult<Self> {
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        // SAFETY: Handles stammen aus der Enumeration; Fehler werden zurückgegeben.
        let item: GraphicsCaptureItem = unsafe {
            match target {
                WgcTarget::Monitor(m) => interop.CreateForMonitor(m)?,
                WgcTarget::Window(h) => interop.CreateForWindow(h)?,
            }
        };
        let dxgi: IDXGIDevice = dev.device.cast()?;
        // SAFETY: gültiges DXGI-Gerät.
        let device: IDirect3DDevice = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? }.cast()?;
        let size = item.Size()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )?;
        let (wake_tx, wake) = bounded::<()>(1);
        let arrived_token = pool.FrameArrived(&TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(
            move |_, _| {
                let _ = wake_tx.try_send(());
                Ok(())
            },
        ))?;
        let (closed_tx, closed) = bounded::<()>(1);
        item.Closed(&TypedEventHandler::<GraphicsCaptureItem, IInspectable>::new(move |_, _| {
            let _ = closed_tx.try_send(());
            Ok(())
        }))?;
        let session = pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsCursorCaptureEnabled(cursor);
        if hide_border {
            // Erst ab Windows 11 verfügbar; Fehler sind hier kein Problem.
            let _ = session.SetIsBorderRequired(false);
        }
        // Ab Windows 11 24H2: WGC liefert nicht öfter als nötig (spart GPU-Kopien).
        let interval = TimeSpan { Duration: 10_000_000 / fps.max(1) as i64 };
        let _ = session.SetMinUpdateInterval(interval);
        session.StartCapture()?;
        Ok(WgcSource {
            pool,
            session,
            _item: item,
            device,
            wake,
            closed,
            current: None,
            size,
            arrived_token,
        })
    }
}

impl FrameSource for WgcSource {
    fn next_frame(&mut self, timeout: Duration) -> CoreResult<Option<SourceFrame>> {
        if self.closed.try_recv().is_ok() {
            return Err(CoreError::Capture("Aufnahmequelle wurde geschlossen".into()));
        }
        // Erst prüfen, ob schon ein Frame wartet; sonst auf das Event warten.
        let mut frame = self.pool.TryGetNextFrame().ok();
        if frame.is_none() {
            match self.wake.recv_timeout(timeout) {
                Ok(()) => frame = self.pool.TryGetNextFrame().ok(),
                Err(RecvTimeoutError::Timeout) => return Ok(None),
                Err(RecvTimeoutError::Disconnected) => return Err(CoreError::NotRunning),
            }
        }
        // Auf den neuesten Frame vorspulen, ältere sofort freigeben.
        while let Ok(newer) = self.pool.TryGetNextFrame() {
            if let Some(old) = frame.replace(newer) {
                let _ = old.Close();
            }
        }
        let Some(frame) = frame else { return Ok(None) };
        let content = frame.ContentSize()?;
        if content.Width != self.size.Width || content.Height != self.size.Height {
            // Fenstergröße geändert: Pool neu anlegen, diesen Frame verwerfen.
            self.size = content;
            let _ = frame.Close();
            self.pool.Recreate(&self.device, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, content)?;
            return Ok(None);
        }
        let surface = frame.Surface()?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
        // SAFETY: Die Surface ist eine D3D11-Textur desselben Geräts.
        let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };
        let pts_us = frame.SystemRelativeTime()?.Duration / 10;
        self.current = Some(frame);
        Ok(Some(SourceFrame { texture, pts_us }))
    }

    fn release(&mut self) {
        if let Some(f) = self.current.take() {
            let _ = f.Close();
        }
    }

    fn size(&self) -> (u32, u32) {
        (self.size.Width.max(0) as u32, self.size.Height.max(0) as u32)
    }
}

impl Drop for WgcSource {
    fn drop(&mut self) {
        self.release();
        let _ = self.pool.RemoveFrameArrived(self.arrived_token);
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
