//! Language detection over the user's captured selection.
//!
//! Used to bind the LLM's output language to the input's. We can't trust
//! a 7B-parameter model to honor a vague "reply in the same language"
//! instruction across all inputs, so we detect the language ourselves
//! and inject the English name (e.g. "Portuguese") into the prompt.

/// Detect the input's language and return its English name
/// (e.g. "English", "Portuguese", "Japanese"). Returns `None` when the
/// input is too short or whatlang isn't confident — callers should fall
/// back to the generic "same language" rule in the system prompt.
pub fn detect(text: &str) -> Option<&'static str> {
    // whatlang's accuracy collapses on very short inputs (a few words can
    // be ambiguous between related languages, e.g. ES/PT/IT). Require at
    // least a sentence's worth of characters before trusting it.
    if text.chars().count() < 12 {
        return None;
    }
    // We intentionally do NOT gate on `info.is_reliable()`. That flag is
    // based on letter-distribution entropy and rejects pangrams and other
    // statistically-odd-but-natural prose. The system prompt also tells
    // the model to "respond in the same language", so even if we hint the
    // wrong language the model has a fallback signal from the input itself.
    whatlang::detect(text).map(|info| info.lang().eng_name())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_english_paragraph() {
        let s = "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs.";
        assert_eq!(detect(s), Some("English"));
    }

    #[test]
    fn detects_portuguese_paragraph() {
        let s = "A raposa marrom rápida salta sobre o cão preguiçoso. Embale a caixa com cinco dúzias de jarras de licor.";
        assert_eq!(detect(s), Some("Portuguese"));
    }

    #[test]
    fn returns_none_for_short_input() {
        assert_eq!(detect("hi"), None);
        assert_eq!(detect("hello world"), None); // 11 chars
    }
}
