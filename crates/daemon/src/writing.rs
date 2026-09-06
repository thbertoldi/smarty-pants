//! Conservative checks for literal facts. This is not a semantic verifier.

use regex::Regex;
use std::{collections::BTreeSet, sync::OnceLock};

pub fn validate(input: &str, output: &str) -> anyhow::Result<()> {
    static NUMBERS: OnceLock<Regex> = OnceLock::new();
    static LINKS: OnceLock<Regex> = OnceLock::new();
    static CODE: OnceLock<Regex> = OnceLock::new();
    let numbers =
        NUMBERS.get_or_init(|| Regex::new(r"[-+]?\d+(?:[.,:/-]\d+)*(?:%|°[CF])?").unwrap());
    let links = LINKS
        .get_or_init(|| Regex::new(r"https?://[^\s<>`]+|[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}").unwrap());
    let code = CODE.get_or_init(|| Regex::new(r"(?s)```.*?```|`[^`\r\n]+`").unwrap());
    for pattern in [numbers, links] {
        let before = extract(pattern, input);
        let after = extract(pattern, output);
        anyhow::ensure!(before == after,
            "rewrite changed or omitted numbers, links, or code; your text was not replaced (writing.preserve_literals)");
    }
    // Allow Markdown to wrap an existing path/URL, while checking that code
    // contents have not changed and no new code has been invented.
    for (source, target) in [(input, output), (output, input)] {
        for fragment in code.find_iter(source) {
            anyhow::ensure!(target.contains(fragment.as_str().trim_matches('`')),
                "rewrite changed or omitted code; your text was not replaced (writing.preserve_literals)");
        }
    }
    Ok(())
}

fn extract<'a>(pattern: &Regex, text: &'a str) -> BTreeSet<&'a str> {
    pattern
        .find_iter(text)
        .map(|m| {
            let value = m.as_str();
            if value.starts_with("http") {
                value.trim_end_matches(['.', ',', ';', '!', '?', ')', ']'])
            } else {
                value
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_missing_or_invented_numbers_and_changed_literals() {
        for (input, output) in [
            ("We tested 24 samples at 18°C.", "We tested samples."),
            ("13 passed, 2 failed", "13 passed, 3 failed"),
            ("latency might improve", "latency might improve by 20%"),
            ("Use https://example.com/v1", "Use https://example.org/v1"),
            ("Keep `retry_count`", "Keep `max_retries`"),
            ("Write to ana@example.com", "Write to bruno@example.com"),
            ("Mantenha 250 ms e 3 tentativas", "Mantenha 250 ms"),
        ] {
            assert!(validate(input, output).is_err(), "{input} -> {output}");
        }
    }

    #[test]
    fn allows_prose_edits_with_unchanged_facts_and_collapsed_repetitions() {
        assert!(validate(
            "Ana send 3 reports. There were 3 reports.",
            "Ana sent 3 reports."
        )
        .is_ok());
        assert!(validate(
            "Veja https://example.com/v1",
            "Consulte https://example.com/v1."
        )
        .is_ok());
        assert!(validate("Eles enviou 13 arquivos", "Eles enviaram 13 arquivos").is_ok());
        assert!(validate(
            "Veja src/client.rs e https://example.com/v1",
            "Veja `src/client.rs` e `https://example.com/v1`"
        )
        .is_ok());
    }
}
