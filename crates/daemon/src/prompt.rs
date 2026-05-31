//! Render a chat-templated prompt for one of the supported templates.

#[derive(Debug, Clone, Copy)]
pub enum Template { Gemma }

pub fn render(template: Template, system: &str, user: &str) -> String {
    match template {
        Template::Gemma => render_gemma(system, user),
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
}
