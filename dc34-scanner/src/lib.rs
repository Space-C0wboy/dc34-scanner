#[cfg(feature = "board-baosec")]
pub mod hid;
pub mod keymap;
pub mod payload;
#[cfg(feature = "board-baosec")]
pub mod scanner;
pub mod state;
#[cfg(feature = "board-baosec")]
pub mod ui;

/// Confirm/cancel buttons, per `KeyMapper::map_keypress` in
/// `bao1x-hal-service/src/servers/keyboard.rs`. The badge's two non-directional
/// "push" inputs both count as confirm so the user doesn't have to know which is
/// which: `🔥` = Center button, `∴` = Select (jog-dial press-in). `←` = back.
/// Any other key shows its raw codepoint on screen (diagnostic) and stays in
/// Preview, so an unexpected mapping never silently cancels.
#[cfg(feature = "board-baosec")]
const FIRE: char = '\u{1F525}'; // 🔥 Center
#[cfg(feature = "board-baosec")]
const SELECT: char = '\u{2234}'; // ∴ Select / jog-dial press
#[cfg(feature = "board-baosec")]
const CANCEL: char = '\u{2190}'; // ← Left = back

/// Construct the badge's IPC clients once, then run the four-state
/// scan -> preview -> confirm -> type loop for good, wiring
/// [`state::Machine`] to the real hardware.
#[cfg(feature = "board-baosec")]
pub fn run_on_hardware() -> ! {
    let xns = xous_names::XousNames::new().unwrap();
    let gfx = ux_api::service::gfx::Gfx::new(&xns).unwrap();
    let usb = usb_bao1x::UsbHid::new();
    let kbd = bao1x_api::keyboard::Keyboard::new(&xns).unwrap();
    let tt = ticktimer_server::Ticktimer::new().unwrap();
    log::info!("scanner up");

    // Real USB connectivity is only known once we try to type (send_str's
    // Err(UseBeforeInit) result), so it is not gated in the state machine.
    let mut m = state::Machine { state: state::State::Idle };

    loop {
        ui::render(&gfx, &m.state);

        match &m.state {
            state::State::Idle => {
                // Any key advances from Idle; the button contract's
                // fire-vs-cancel distinction only matters once there's
                // something to confirm or cancel (Preview).
                let _ = ui::next_button(&kbd);
                m.step(state::Event::ScanButton);
            }
            state::State::Scanning => {
                // acquire_qr() blocks internally and self-aborts (returns
                // content: None) on any button press, replacing our
                // ScanTimeout event -- that transition stays in the
                // machine but is unused here.
                //
                // Settle delay first: the keypress that triggered this scan is
                // ALSO broadcast to the bao-video Gfx service, whose KeyPress
                // handler aborts a pending acquire_qr. Without this delay, our
                // AcquireQr can be enqueued at bao-video just before that trigger
                // KeyPress, so the scan self-aborts the instant the camera starts
                // (symptom: "Scanning" -> wait icon -> straight back to idle,
                // camera never really runs). Sleeping here guarantees the trigger
                // KeyPress is enqueued and processed by bao-video (a no-op, since
                // no scan is pending yet) BEFORE our AcquireQr arrives.
                tt.sleep_ms(500).ok();
                match scanner::scan(&gfx) {
                    Some(s) => m.step(state::Event::QrDecoded(s)),
                    None => m.step(state::Event::CancelButton),
                }
            }
            state::State::Preview(_) => {
                let k = ui::next_button(&kbd);
                if k == FIRE || k == SELECT {
                    m.step(state::Event::ConfirmButton);
                } else if k == CANCEL {
                    m.step(state::Event::CancelButton);
                } else {
                    // Unknown button: show what it reported instead of silently
                    // cancelling, so a surprising key mapping is visible. Stay in
                    // Preview (no state step).
                    ui::render_message(
                        &gfx,
                        &format!("key U+{:04X}\nmiddle/jog=send  left=back", k as u32),
                    );
                    tt.sleep_ms(1500).ok();
                }
            }
            state::State::Typing(payload) => {
                let payload = payload.clone();
                match hid::type_payload(&usb, &payload) {
                    Ok(()) => {
                        m.step(state::Event::TypingComplete);
                    }
                    Err(e) => {
                        log::warn!("send_str failed, treating as USB not connected: {:?}", e);
                        ui::render_message(&gfx, "USB not connected");
                        m.step(state::Event::UsbLost);
                    }
                }
            }
        }
    }
}

pub fn crate_is_wired() -> bool { true }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaffold_compiles() {
        assert!(crate_is_wired());
    }
}
