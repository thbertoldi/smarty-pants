//! Render a chat-templated prompt for one of the supported templates.

#[derive(Debug, Clone, Copy)]
pub enum Template {
    Gemma,
    /// ChatML — used by Qwen 2.5 / Qwen 3 / Yi / many other instruction-tuned
    /// models. Has a proper `system` role distinct from user.
    ChatML,
}

/// Render a templated prompt. `language` is the English name of the
/// detected input language (e.g. "Portuguese"). When supplied, the
/// template adds an explicit pre-input statement AND a post-input
/// reminder — a vague "same language" rule alone isn't reliable on
/// 7B-class models. Pass `None` to fall back to the system prompt's
/// generic language rule (used when language detection isn't confident).
pub fn render(
    template: Template,
    system:   &str,
    user:     &str,
    language: Option<&str>,
) -> String {
    match template {
        Template::Gemma  => render_gemma(system, user, language),
        Template::ChatML => render_chatml(system, user, language),
    }
}

fn lang_pre(language: Option<&str>) -> String {
    match language {
        Some(lang) => format!("The input below is in {lang}. Your output MUST also be in {lang}.\n\n"),
        None => String::new(),
    }
}

fn lang_post(language: Option<&str>) -> String {
    match language {
        Some(lang) => format!("\n\nReminder: respond in {lang}."),
        None => String::new(),
    }
}

/// Gemma 2/3 chat template — no system role, so we inject the system
/// instructions and the user text together as a single user turn. The
/// user's text is wrapped in `<input>...</input>` tags so the model
/// reliably treats it as opaque content (instruction-injection guard)
/// rather than as a request directed at the assistant.
fn render_gemma(system: &str, user: &str, language: Option<&str>) -> String {
    let pre  = lang_pre(language);
    let post = lang_post(language);
    format!(
        "<start_of_turn>user\n{system}\n\n{pre}<input>\n{user}\n</input>{post}<end_of_turn>\n<start_of_turn>model\n",
        system = system.trim(),
        user   = user.trim()
    )
}

/// ChatML — Qwen 2.5 / Qwen 3 / Yi format. Separate system role; user text
/// wrapped in `<input>...</input>` as an instruction-injection guard.
fn render_chatml(system: &str, user: &str, language: Option<&str>) -> String {
    let pre  = lang_pre(language);
    let post = lang_post(language);
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{pre}<input>\n{user}\n</input>{post}<|im_end|>\n<|im_start|>assistant\n",
        system = system.trim(),
        user   = user.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemma_snapshot() {
        let out = render(
            Template::Gemma,
            "Rewrite in different words. Same meaning. Same language.",
            "The quick brown fox jumps over the lazy dog.",
            None,
        );
        insta::assert_snapshot!(out);
    }

    #[test]
    fn gemma_with_language_snapshot() {
        let out = render(
            Template::Gemma,
            "Rewrite in different words.",
            "A raposa marrom rápida salta sobre o cão preguiçoso.",
            Some("Portuguese"),
        );
        insta::assert_snapshot!(out);
    }

    #[test]
    fn chatml_snapshot() {
        let out = render(
            Template::ChatML,
            "Rewrite in different words. Same meaning. Same language.",
            "The quick brown fox jumps over the lazy dog.",
            None,
        );
        insta::assert_snapshot!(out);
    }

    #[test]
    fn chatml_with_language_snapshot() {
        let out = render(
            Template::ChatML,
            "Rewrite in different words.",
            "A raposa marrom rápida salta sobre o cão preguiçoso.",
            Some("Portuguese"),
        );
        insta::assert_snapshot!(out);
    }
}
