use crate::keymap;

/// Summary of a decoded QR payload, for the preview screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadInfo {
    pub char_count: usize,
    pub untypeable_count: usize,
    pub preview: String,
    /// The original, untruncated payload text. Not part of the public
    /// "preview info" contract (crate-private), but kept so the state
    /// machine's `Typing` state can carry the real decoded string rather
    /// than the possibly-truncated `preview`.
    pub(crate) raw: String,
}

/// Analyze `payload`: count its characters, count how many of them can't
/// be typed via [`keymap::is_typeable`], and build a preview string
/// truncated to `max_preview_chars` (with a `…(+N more)` suffix when the
/// payload is longer than that).
pub fn analyze(payload: &str, max_preview_chars: usize) -> PayloadInfo {
    let char_count = payload.chars().count();
    let untypeable_count = payload.chars().filter(|c| !keymap::is_typeable(*c)).count();

    let preview = if char_count > max_preview_chars {
        let truncated: String = payload.chars().take(max_preview_chars).collect();
        let more = char_count - max_preview_chars;
        format!("{truncated}…(+{more} more)")
    } else {
        payload.to_string()
    };

    PayloadInfo { char_count, untypeable_count, preview, raw: payload.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_and_short_preview() {
        let info = analyze("https://x.com", 100);
        assert_eq!(info.char_count, 13);
        assert_eq!(info.untypeable_count, 0);
        assert_eq!(info.preview, "https://x.com");
    }
    #[test]
    fn flags_untypeable() {
        let info = analyze("wifi:é", 100);
        assert_eq!(info.untypeable_count, 1);
    }
    #[test]
    fn truncates_long_payload() {
        let info = analyze(&"a".repeat(50), 10);
        assert!(info.preview.starts_with(&"a".repeat(10)));
        assert!(info.preview.contains("+40 more"));
    }
}
