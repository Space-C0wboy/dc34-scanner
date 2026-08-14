//! Display rendering (per-state screens) and button reading.

use core::fmt::Write as TextViewWrite;

use bao1x_api::keyboard::Keyboard;
use blitstr2::GlyphStyle;
use ux_api::minigfx::*;
use ux_api::service::api::Gid;
use ux_api::service::gfx::Gfx;

use crate::state::State;

/// Draw the screen for the current state and commit it to the physical
/// display. IDLE / PREVIEW / TYPING each get a purpose-built screen;
/// SCANNING gets a simple "Scanning…" screen even though in practice
/// `Gfx::acquire_qr()` blocks the caller before `render` is called again
/// for it.
pub fn render(gfx: &Gfx, state: &State) {
    let screen = gfx.screen_size().unwrap_or(Point::new(128, 128));
    let bounds = Rectangle::new(Point::new(0, 0), screen);

    let mut tv = TextView::new(Gid::dummy(), TextBounds::CenteredTop(bounds));
    tv.style = GlyphStyle::Regular;
    tv.draw_border = false;

    match state {
        State::Idle => {
            write!(tv, "Scan a QR - press any key").ok();
        }
        State::Scanning => {
            write!(tv, "Scanning...").ok();
        }
        State::Preview(info) => {
            write!(tv, "{}\n\n{} chars", info.preview, info.char_count).ok();
            if info.untypeable_count > 0 {
                write!(
                    tv,
                    "\n\n{} chars can't be typed\n\nmiddle/jog=send  left=back",
                    info.untypeable_count
                )
                .ok();
            } else {
                write!(tv, "\n\nmiddle/jog=send  left=back").ok();
            }
        }
        State::Typing(_) => {
            write!(tv, "Typing...").ok();
        }
    }

    gfx.clear().ok();
    // A transient draw failure should not crash the app; the next loop
    // iteration re-renders.
    gfx.draw_textview(&mut tv).ok();
    gfx.flush().ok();
}

/// Draw a single ad-hoc message screen (e.g. a transient "USB not
/// connected" notice) outside the normal per-`State` screens in [`render`].
pub fn render_message(gfx: &Gfx, msg: &str) {
    let screen = gfx.screen_size().unwrap_or(Point::new(128, 128));
    let bounds = Rectangle::new(Point::new(0, 0), screen);

    let mut tv = TextView::new(Gid::dummy(), TextBounds::CenteredTop(bounds));
    tv.style = GlyphStyle::Regular;
    tv.draw_border = false;
    write!(tv, "{}", msg).ok();

    gfx.clear().ok();
    gfx.draw_textview(&mut tv).ok();
    gfx.flush().ok();
}

/// Block until a button is pressed and return the first char reported.
/// Per the button contract: `🔥` (center/fire) means confirm/advance, any
/// other key means cancel/back.
pub fn next_button(kbd: &Keyboard) -> char {
    loop {
        let keys = kbd.get_keys_blocking();
        if let Some(k) = keys.into_iter().next() {
            return k;
        }
    }
}
