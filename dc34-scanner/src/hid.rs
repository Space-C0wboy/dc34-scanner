//! USB HID typing: sends a decoded QR payload as keystrokes via
//! `UsbHid::send_str`.

use crate::keymap;

/// Type `payload` over USB HID.
///
/// Filters `payload` down to only the characters [`keymap::is_typeable`]
/// says the real hardware can emit (this is the "skip-and-warn"
/// enforcement point: any untypeable character is silently dropped from
/// what's sent, matching the count already shown to the user on the
/// preview screen), sets the autotype pacing, then sends the filtered
/// string in one `send_str` call.
///
/// A `send_str` `Err(UseBeforeInit)` means USB likely isn't connected to a
/// host; that's surfaced as-is so the caller can report "USB not
/// connected" and route back to Idle.
pub fn type_payload(usb: &usb_bao1x::UsbHid, payload: &str) -> Result<(), xous::Error> {
    let filtered: String = payload.chars().filter(|c| keymap::is_typeable(*c)).collect();

    // 30ms is the firmware default and the value vault uses for password
    // autotype. An earlier 8ms setting garbled output on real hardware: the
    // USB host polls slower than the report changes, so key-down/key-up
    // transitions get missed and merged (the usb-bao1x SendString handler's
    // own comment warns fast rates "cause repeated characters"). Keep 30ms.
    usb.set_autotype_delay_ms(30);

    usb.send_str(&filtered)?;
    Ok(())
}
