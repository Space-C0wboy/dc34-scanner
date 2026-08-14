use crate::payload::{self, PayloadInfo};

/// Maximum number of characters kept in a payload preview (see
/// [`payload::analyze`]).
pub const PREVIEW_MAX: usize = 120;

/// The scanner's four-state UI flow.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Idle,
    Scanning,
    Preview(PayloadInfo),
    Typing(String),
}

/// Inputs that drive [`Machine::step`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ScanButton,
    ConfirmButton,
    CancelButton,
    QrDecoded(String),
    ScanTimeout,
    UsbLost,
    TypingComplete,
}

/// Hardware-independent scanner state machine.
pub struct Machine {
    pub state: State,
}

impl Machine {
    /// Pure transition function; no I/O. See the transition table in
    /// `phase1-brief.md` Task 1.4 for the full rule set.
    ///
    /// Note: confirming a preview always advances to `Typing`. USB
    /// connectivity is not knowable ahead of time on this board, so it is
    /// not gated here — the actual `send_str` result during `Typing` is the
    /// sole source of truth, and a failure drives `Event::UsbLost` back to
    /// `Idle`.
    pub fn step(&mut self, ev: Event) {
        let current = std::mem::replace(&mut self.state, State::Idle);
        self.state = match (current, ev) {
            (State::Idle, Event::ScanButton) => State::Scanning,

            (State::Scanning, Event::QrDecoded(s)) => {
                State::Preview(payload::analyze(&s, PREVIEW_MAX))
            }
            (State::Scanning, Event::CancelButton) => State::Idle,
            (State::Scanning, Event::ScanTimeout) => State::Idle,

            (State::Preview(info), Event::ConfirmButton) => {
                // Typing carries the original decoded string, not the
                // (possibly truncated) preview text.
                State::Typing(info.raw.clone())
            }
            (State::Preview(_), Event::CancelButton) => State::Idle,

            (State::Typing(_), Event::TypingComplete) => State::Idle,
            (State::Typing(_), Event::UsbLost) => State::Idle,

            (other, _) => other,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m() -> Machine { Machine { state: State::Idle } }

    #[test]
    fn idle_scan_goes_scanning() {
        let mut x = m();
        x.step(Event::ScanButton);
        assert!(matches!(x.state, State::Scanning));
    }
    #[test]
    fn decode_goes_preview() {
        let mut x = m(); x.step(Event::ScanButton);
        x.step(Event::QrDecoded("hi".into()));
        assert!(matches!(x.state, State::Preview(_)));
    }
    #[test]
    fn confirm_always_types() {
        // USB is not gated in the FSM; confirming a preview always advances
        // to Typing, where the send result is the real USB check.
        let mut x = m(); x.step(Event::ScanButton);
        x.step(Event::QrDecoded("hi".into()));
        x.step(Event::ConfirmButton);
        assert!(matches!(x.state, State::Typing(_)));
    }
    #[test]
    fn typing_carries_raw_not_preview() {
        // A payload longer than PREVIEW_MAX must be typed in full, not the
        // truncated preview text.
        let long = "x".repeat(PREVIEW_MAX + 20);
        let mut x = m(); x.step(Event::ScanButton);
        x.step(Event::QrDecoded(long.clone()));
        x.step(Event::ConfirmButton);
        match x.state {
            State::Typing(s) => assert_eq!(s, long),
            other => panic!("expected Typing, got {:?}", other),
        }
    }
    #[test]
    fn cancel_from_preview_returns_idle() {
        let mut x = m(); x.step(Event::ScanButton);
        x.step(Event::QrDecoded("hi".into()));
        x.step(Event::CancelButton);
        assert!(matches!(x.state, State::Idle));
    }
    #[test]
    fn scan_timeout_returns_idle() {
        let mut x = m(); x.step(Event::ScanButton);
        x.step(Event::ScanTimeout);
        assert!(matches!(x.state, State::Idle));
    }
    #[test]
    fn usb_lost_mid_typing_returns_idle() {
        let mut x = m(); x.step(Event::ScanButton);
        x.step(Event::QrDecoded("hi".into())); x.step(Event::ConfirmButton);
        x.step(Event::UsbLost);
        assert!(matches!(x.state, State::Idle));
    }
}
