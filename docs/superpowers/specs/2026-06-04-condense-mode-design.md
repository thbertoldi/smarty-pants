# Condense mode — design

**Status:** approved 2026-06-04
**Owner:** thbertoldi
**Type:** feature (new default mode)

## Motivation

When typing prompts to feed into large language models (Claude, ChatGPT,
local LLMs, …) the input is often verbose: pleasantries, hedges,
redundant articulations of the same constraint. Those tokens cost money
on hosted APIs and waste context window on local models.

Add a fourth default mode, **`condense`**, that rewrites highlighted
text into a token-efficient, telegraphic form while preserving meaning.
Workflow: highlight your draft prompt → press `Super+K` → paste the
condensed version into the LLM of choice.

Same surface area as the existing three modes (`rewrite`, `academic`,
`linkedin`). Same pipeline, same portal integration, same CLI. The only
new thing is the prompt and one shortcut binding.

## Scope

### In

- New mode named `condense`, registered as a default alongside the
  existing three.
- New system prompt at `examples/prompts/condense.txt` tuned for
  aggressive prompt-style compression: telegraphic, imperative, drop
  articles/pleasantries where safe, prefer shorter synonyms.
- Hard correctness guard in the prompt: code blocks, inline code,
  identifiers, numbers, dates, URLs, and quoted strings MUST be
  preserved verbatim. Mangling those would silently break user prompts
  (e.g. shortening `def even_sum` to `def es`) — this is not a style
  choice, it is a correctness constraint.
- One entry added to `DEFAULT_MODES` in `crates/daemon/src/main.rs`
  binding the new mode to suggested shortcut `SUPER+K`.
- README updates: shortcut table row, Hyprland bind line, niri KDL
  binding, Sway bindsym line, Usage section step 3.

### Out

- Token-counter integration ("saved 42% tokens" feedback in the daemon
  log or CLI output). Nice future feature; orthogonal.
- Tokenizer-aware compression (optimizing for GPT-4o BPE vs Claude
  tokenizer vs Llama). Out of scope; the prompt-level approach is
  tokenizer-agnostic by design.
- A `preserve_structure` config knob. The prompt itself handles
  structure preservation; no flag needed.
- Per-mode sampling overrides (`temperature`, `top_p`). The default
  `[model]` sampling is used; if v1 testing shows the prose-aggression
  is too unpredictable, tuning is a follow-up.

## Design

### Files touched

| File | Change |
|------|--------|
| `examples/prompts/condense.txt` | NEW — system prompt (≈30 lines) |
| `crates/daemon/src/main.rs` | one tuple appended to `DEFAULT_MODES` |
| `README.md` | shortcut table row, three bind snippets, Usage step 3 |

No other source file is touched. The mode system is data-driven; the
daemon, pipeline, portal session, dispatcher, CLI, and config types all
work generically over `Config.modes`.

### Suggested hotkey

- README documents `Super+K` as the suggested key combo. Rationale:
  free in stock Hyprland, doesn't collide with stock app shortcuts
  (`Super+C` would collide with Copy in most apps), mnemonic stretch as
  "K for kondense / kompact".
- Users always override in their own compositor config; `Super+K` is
  only the documented default, not a hard requirement.

### System prompt shape

Follows the structure of the existing three prompts (see
`examples/prompts/rewrite.txt`):

1. Role declaration ("You are a text condenser…").
2. Bullet list of style rules (telegraphic, imperative, drop fillers,
   shorter synonyms, same language).
3. Bullet list of verbatim-preservation rules (code, identifiers,
   numbers, dates, URLs, quoted strings).
4. `CRITICAL RULES` block — copies the instruction-injection guard and
   "output only the improved text" guard from the other prompts.

The exact wording lands in the implementation plan; the design fixes
the *structure* and *behavioural contract*, not the literal text.

### Pipeline behaviour (unchanged)

- Portal activation event for shortcut id `condense` → `Dispatcher`
  → `Pipeline.run("condense")` → look up `cfg.modes["condense"]` →
  render with the configured chat template → run the LLM → write
  to clipboard and synthesize paste. Identical to the existing modes.
- Language detection (`whatlang`) runs as usual; same-language guard in
  the prompt applies.

## Testing

Manual smoke after build:

1. Start daemon → confirm log line `portal shortcuts bound count=4`.
2. Highlight a verbose English prompt → `Super+K` → result is
   meaningfully shorter, preserves identifiers / numbers / URLs
   verbatim, in English.
3. Highlight a Portuguese sentence → `Super+K` → condensed output
   stays in Portuguese.
4. CLI parity: `smarty-pants trigger --mode condense` produces the
   same behaviour as the portal hotkey.
5. Highlight a prompt containing a fenced code block → `Super+K` →
   code block emerges unchanged.

No automated test additions in v1 — the snapshot tests in `prompt.rs`
cover template rendering, and the integration paths exercised by adding
a fourth mode are identical to the existing three.

## Risks & mitigations

- **Model mangles code/identifiers.** Mitigated by the explicit
  verbatim-preservation block in the prompt. If drift is observed
  during smoke testing, a follow-up change can lower variance by
  setting a per-mode `temperature: 0.3` on the default entry — that
  knob is already supported by `ModeCfg`, no schema change required.
- **Compression too lossy.** v1 ships aggressive style by user
  preference. If smoke testing shows the model regularly drops
  semantic content, the fallback is to rewrite the prompt in the
  softer "tight prose" form considered during brainstorming and
  re-test before declaring the mode done.
- **Shortcut collision in user's existing compositor config.** README
  documentation already follows the established pattern of "pick
  whatever you have free" — same as the `Super+I` note for LinkedIn.
