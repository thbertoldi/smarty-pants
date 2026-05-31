//! Render a chat-templated prompt for one of the supported templates.

#[derive(Debug, Clone, Copy)]
pub enum Template {
    Gemma,
    /// ChatML — used by Qwen 2.5 / Qwen 3 / Yi / many other instruction-tuned
    /// models. Has a proper `system` role distinct from user.
    ChatML,
}

pub fn render(template: Template, system: &str, user: &str) -> String {
    match template {
        Template::Gemma  => render_gemma(system, user),
        Template::ChatML => render_chatml(system, user),
    }
}

/// Gemma 2/3 chat template — no system role, so we inject the system
/// instructions and the user text together as a single user turn. The
/// user's text is wrapped in `<input>...</input>` tags so the model
/// reliably treats it as opaque content (instruction-injection guard)
/// rather than as a request directed at the assistant.
fn render_gemma(system: &str, user: &str) -> String {
    format!(
        "<start_of_turn>user\n{system}\n\n<input>\n{user}\n</input><end_of_turn>\n<start_of_turn>model\n",
        system = system.trim(),
        user   = user.trim()
    )
}

/// ChatML — Qwen 2.5 / Qwen 3 / Yi format. Separate system role; user text
/// wrapped in `<input>...</input>` as an instruction-injection guard.
fn render_chatml(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n<input>\n{user}\n</input><|im_end|>\n<|im_start|>assistant\n",
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
        );
        insta::assert_snapshot!(out);
    }

    #[test]
    fn chatml_snapshot() {
        let out = render(
            Template::ChatML,
            "Rewrite in different words. Same meaning. Same language.",
            "The quick brown fox jumps over the lazy dog.",
        );
        insta::assert_snapshot!(out);
    }
}
