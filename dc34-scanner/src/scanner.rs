//! QR scanning: a thin wrapper over `Gfx::acquire_qr()`.
//!
//! `Gfx::acquire_qr()` is a blocking IPC call into the `bao-video` service:
//! it drives the camera, decodes a QR code server-side, and blocks until
//! either a code is decoded or the user aborts by pressing any button (in
//! which case it returns `Ok(QrAcquisition { content: None, .. })`, not an
//! `Err`). We fold both "hard" failure (`Err`) and "aborted by button"
//! (`Ok(None)`) into a single `None` return here, since from the scanner
//! state machine's point of view both mean "go back to Idle".

use ux_api::service::gfx::Gfx;

/// Block until a QR code is scanned or the user aborts. Returns the decoded
/// payload string, or `None` on abort/failure (caller should treat this as
/// a `CancelButton` event and return to `Idle`).
pub fn scan(gfx: &Gfx) -> Option<String> {
    match gfx.acquire_qr() {
        Ok(qr_data) => {
            if let Some(meta) = &qr_data.meta {
                log::info!("QR code metadata: {}", meta);
            }
            qr_data.content
        }
        Err(e) => {
            log::error!("QR acquisition failed: {:?}", e);
            None
        }
    }
}
