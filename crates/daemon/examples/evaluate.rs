//! Opt-in writing evaluation. Uses synthetic fixtures; never touches the clipboard.

use serde::Deserialize;
use serde_json::{json, Value};
use smarty_pants_core::config::Config;
use smarty_pants_daemon::{backend, language, llm::GenerationParams, prompt::Prompt};
use std::{collections::HashSet, io::Write, path::Path, time::Instant};

#[derive(Deserialize)]
struct Case {
    id: String,
    language: String,
    mode: String,
    text: String,
    #[serde(default)]
    must_preserve: Vec<String>,
    #[serde(default)]
    must_not_contain: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--assess") {
        anyhow::ensure!(
            args.len() == 3,
            "usage: evaluate --assess <cases.jsonl> <results.jsonl>"
        );
        return assess_file(&args[1], &args[2]);
    }
    anyhow::ensure!(
        args.len() == 2 || (args.len() == 3 && args[2] == "--warmup"),
        "usage: evaluate <config.toml> <cases.jsonl> [--warmup]"
    );
    let mut cfg = Config::from_path(Path::new(&args[0]))?;
    cfg.add_builtin_modes();
    let llm = backend::create(&cfg)?;
    let model_name = cfg
        .api_config()
        .map(|api| api.model)
        .unwrap_or_else(|| cfg.model.name.clone());
    let cases = load_cases(&args[1])?;
    let warmed_up = args.len() == 3;
    if warmed_up {
        let prompt = Prompt::new(
            &cfg.modes["rewrite"].system,
            "This sentence have a small grammatical error.",
            Some("English"),
        );
        let start = Instant::now();
        let output = llm
            .generate(
                &prompt,
                &GenerationParams {
                    temperature: 0.0,
                    seed: 42,
                    ..Default::default()
                },
            )
            .await?;
        eprintln!(
            "{}",
            json!({"warmup_ms_including_load": start.elapsed().as_millis(), "output": output})
        );
    }
    let mut failed = false;
    for (index, case) in cases.iter().enumerate() {
        let mode = cfg
            .modes
            .get(&case.mode)
            .ok_or_else(|| anyhow::anyhow!("unknown mode {}", case.mode))?;
        let prompt = Prompt::new(&mode.system, &case.text, language::detect(&case.text));
        let params = GenerationParams {
            max_tokens: mode.max_tokens.unwrap_or(cfg.model.max_tokens),
            temperature: mode.temperature.unwrap_or(cfg.model.temperature),
            top_p: mode.top_p.unwrap_or(cfg.model.top_p),
            seed: cfg.model.seed,
        };
        let start = Instant::now();
        let result = llm.generate(&prompt, &params).await;
        let ms = start.elapsed().as_millis();
        let mut row = json!({
            "id": case.id, "mode": case.mode, "expected_language": case.language,
            "input": case.text, "model": model_name, "provider": cfg.inference.provider, "ms": ms,
            "system": prompt.system, "user_message": prompt.user_message(),
            "generation": {"temperature": params.temperature, "top_p": params.top_p,
                "seed": params.seed, "max_tokens": params.max_tokens},
            "first_request_includes_load": !warmed_up && index == 0,
            "rss_kib": memory_kib("VmRSS:"), "peak_rss_kib": memory_kib("VmHWM:")
        });
        match result {
            Ok(output) => row["output"] = json!(output),
            Err(error) => {
                failed = true;
                row["error"] = json!(error.to_string());
            }
        };
        assess(case, &mut row, cfg.writing.preserve_literals);
        println!("{row}");
        std::io::stdout().flush()?;
    }
    llm.unload().await;
    anyhow::ensure!(
        !failed,
        "one or more cases failed; see the JSONL error records"
    );
    Ok(())
}

fn load_cases(path: &str) -> anyhow::Result<Vec<Case>> {
    let corpus = std::fs::read_to_string(path)?;
    let cases: Vec<Case> = corpus
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let mut ids = HashSet::new();
    anyhow::ensure!(!cases.is_empty(), "empty corpus");
    for case in &cases {
        anyhow::ensure!(ids.insert(&case.id), "duplicate case ID {}", case.id);
    }
    Ok(cases)
}

fn assess(case: &Case, row: &mut Value, preserve_literals: bool) {
    let Some(output) = row["output"].as_str() else {
        return;
    };
    let missing: Vec<_> = case
        .must_preserve
        .iter()
        .filter(|s| !output.contains(s.as_str()))
        .collect();
    let unexpected: Vec<_> = case
        .must_not_contain
        .iter()
        .filter(|s| output.to_lowercase().contains(&s.to_lowercase()))
        .collect();
    let fidelity_error = preserve_literals
        .then(|| {
            smarty_pants_daemon::writing::validate(&case.text, output)
                .err()
                .map(|e| e.to_string())
        })
        .flatten();
    let assessment = json!({"detected_language": language::detect(output),
        "missing": missing, "unexpected": unexpected, "fidelity_error": fidelity_error});
    row.as_object_mut()
        .unwrap()
        .extend(assessment.as_object().unwrap().clone());
}

// Apply exactly the same language/literal checks to externally generated outputs.
// Validate the entire input before writing anything so partial/mismatched runs fail.
fn assess_file(corpus: &str, results: &str) -> anyhow::Result<()> {
    let cases = load_cases(corpus)?;
    let raw = std::fs::read_to_string(results)?;
    let mut rows: Vec<Value> = raw
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    anyhow::ensure!(rows.len() == cases.len(), "result/corpus count mismatch");
    let mut seen = HashSet::new();
    for row in &mut rows {
        let id = row["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing result ID"))?
            .to_owned();
        anyhow::ensure!(seen.insert(id.clone()), "duplicate result ID {id}");
        let case = cases
            .iter()
            .find(|case| case.id == id)
            .ok_or_else(|| anyhow::anyhow!("unknown result ID {id}"))?;
        anyhow::ensure!(
            row["input"].as_str() == Some(&case.text),
            "input mismatch for {id}"
        );
        anyhow::ensure!(
            row["output"].is_string() || row["error"].is_string(),
            "missing output/error for {id}"
        );
        assess(case, row, true);
    }
    for row in rows {
        println!("{row}");
    }
    Ok(())
}

fn memory_kib(field: &str) -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix(field)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}
