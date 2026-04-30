# Implementation Plan: redact

## Approach

Build the project bottom-up across four milestones: foundation (config + patterns) → Gate 2 (the load-bearing safety net) → Gate 1 + integration (`redact run`) → hook surface (`hook` + `init` + `config`). Each milestone ends in a runnable, testable state. The first three are useful even before the hook layer ships — `redact run` can be exercised manually for end-to-end testing without any harness integration.

This ordering puts the highest-risk component (Gate 2's correctness on real data) earliest and the harness-coupled components (which are the most fragile to test) last. If Gate 2 has problems, we want to know in week one, not week three.

**Before the milestones: build a prototype first.** See the Prototype section below.

---

## Prototype (before Milestone 1)

Goal: prove the end-to-end flow works inside Claude Code in a few hours, before investing in the full implementation.

**What to build:**

- `redact hook` — reads the Bash command from stdin; if `argv[0]` matches a hardcoded tool list (`tkpsql`, `tkdbr`, `mysql`, `psql`), rewrites to `redact run -- <original command>`; otherwise passes through unchanged.
- `redact run` — spawns the subprocess, captures stdout, runs Gate 2 with hardcoded PII patterns (email, SSN, phone, credit card via Luhn), prints redacted JSON to stdout.

**What to skip:**

- Gate 1 (SQL inspection) — Gate 2 alone proves the safety net
- `redact init` — manually insert the hook entry into `~/.claude/settings.json`
- `redact config`, `redact list`, `redact validate` — hardcode tool list and patterns
- Full config system, harness detection, atomic writes, error handling

**Hook entry to install manually:**

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "redact hook" }] }
    ]
  }
}
```

**Exit criterion:** inside a live Claude Code session, ask the AI to run a query tool that returns JSON with PII — observe the redacted output returned to the model. Gate 2 must catch email, SSN, and phone in a realistic JSON payload. Once this works, proceed to Milestone 1 and replace the prototype with the production implementation.

---

## Repository setup

**Step 1.** Create the Cargo workspace:

```
redact/
  Cargo.toml                 # workspace root
  crates/
    common/Cargo.toml        # config, patterns, redactor, error, harness
    gate1/Cargo.toml         # SQL tokenizer + column extractor
    redact/Cargo.toml        # main binary
```

**Step 2.** Pin dependencies in workspace `Cargo.toml`:

- `clap = { version = "4", features = ["derive"] }`
- `serde = { version = "1", features = ["derive"] }`
- `serde_json = { version = "1", features = ["preserve_order"] }`
- `serde_yaml = "0.9"`
- `regex = "1"`
- `shell-words = "1"`
- `anyhow = "1"`, `thiserror = "1"`
- `tempfile = "3"` (test-only)

**Step 3.** Set up CI: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --all`.

---

## Milestone 1 — Foundation (`common` crate)

Goal: config loads cleanly, patterns compile, errors render correctly. No CLI yet.

**Step 4. `common/error.rs`** — `ErrorResponse` struct (serializes to `{"error": "..."}`), `exit_with_error(msg)` helper that prints the JSON to stdout and exits 1.

**Step 5. `common/config.rs`** — Config types:

```rust
pub struct Config {
    pub tools: HashMap<String, ToolConfig>,
    pub pii: PiiConfig,
}
pub struct ToolConfig { pub sql_arg: Option<String> }
pub struct PiiConfig {
    pub column_names: Vec<String>,        // merged with built-in defaults
    pub action: Action,                    // Warn | Redact | Reject
    pub wildcard_policy: WildcardPolicy,   // Warn | Reject
    pub patterns: HashMap<String, Pattern>,
    pub column_name_boost: f32,            // default 0.15
    pub confidence_threshold: f32,         // default 0.8
    pub redaction: String,                 // default "[PII:{type}]"
    pub include_summary: bool,             // default true
}
pub struct Pattern { pub regex: String, pub confidence: f32 }
```

Implement `Config::load()` that resolves the path from `REDACT_CONFIG` → `~/.config/redact/config.yaml`, merges defaults with user values (so users only specify overrides), and returns a typed error for parse failures.

**Step 6. `common/patterns.rs`** — Built-in defaults baked in: column-name denylist (`email`, `ssn`, `dob`, `phone`, `npi`, `credit_card`, `card_number`, `cvv`, `passport`, `license_number`, `full_name`, `first_name`, `last_name`, `birthdate`), regex defaults with their base confidences, `column_name_boost = 0.15`, `confidence_threshold = 0.8`. `CompiledPatterns` struct holds compiled `Regex` + score + name. `Luhn::check(&str) -> bool` for credit cards.

**Step 7. `common/harness.rs`** — `is_agent_harness() -> bool` checks `CLAUDECODE`, `OPENCODE`, `COPILOT_CLI`, `COPILOT_RUN_APP`.

**Step 8. Tests** — Config: round-trip parse, defaults merge, missing file error, malformed YAML error. Patterns: each built-in regex matches its golden corpus and rejects negative cases. Luhn: valid/invalid card test vectors.

**Exit criterion:** `cargo test -p common` passes; manual `Config::load()` from a sample YAML returns the expected struct.

---

## Milestone 2 — Gate 2 (`common/redactor.rs`)

Goal: given a JSON payload + `RedactPlan`, return a redacted JSON payload + summary. This is the highest-risk component — wrong here means PII leaks.

**Step 9. `RedactPlan` struct** in `common/`:

```rust
pub struct RedactPlan {
    pub forced_columns: HashMap<String, String>,  // lowercased key → type label
    pub warnings: Vec<String>,
    pub rejected: bool,
}
impl RedactPlan { pub fn empty() -> Self { ... } }
```

**Step 10. Shape detection** — `detect_shape(&Value) -> Shape` enum (`Error | Object | Array | Other`). Error means top-level object with an `error` key.

**Step 11. Tree walk** — recursive function that visits every `(key, value)` pair. For string leaves, build a `Vec<Match>` with `(start, end, type, confidence)` from regex + Luhn + forced-column check. Pick the highest-confidence match per value. If `confidence >= threshold`, replace the value with the redaction template; otherwise add a low-confidence warning to the summary. Forced-column matches always win, score = 1.0, no regex run.

**Step 12. Summary attachment** — `apply_summary(payload, summary, include_summary, shape)`:

- `Error` → unchanged.
- `Object` → set `payload["_redact_summary"] = summary` if enabled.
- `Array` + enabled → wrap as `{"rows": payload, "_redact_summary": summary}`.
- `Array` + disabled → unchanged.
- `Other` → unchanged.

**Step 13. Use `serde_json` with `preserve_order`** so column order in output matches input (NFR-4).

**Step 14. Tests** — Golden-file tests with input/output JSON pairs covering: object with `rows`, bare array, error pass-through, nested JSONB, columns with PII keys but null values, multiple matches in one string (highest confidence wins), forced column from `RedactPlan` (regex doesn't run), low-confidence match (warned but not redacted), Luhn-passes (always redact regardless of column), Luhn-fails on a 16-digit non-card string. Property test: redaction is idempotent (running redact twice = running once).

**Exit criterion:** Hand-craft 8–10 sample query result files (mix of toolkit-shaped and `mysql --json`-shaped, with realistic PII), assert correct redaction. False-negative rate on the test corpus = 0.

---

## Milestone 3 — Gate 1 (`gate1` crate)

Goal: extract column names from a SQL string. Best-effort.

**Step 15. `gate1/tokenizer.rs`** — Hand-written SQL tokenizer that recognizes: identifiers, quoted identifiers (`"col"`, `` `col` ``), commas, parens, dots, whitespace, comments (`--`, `/* */`), keywords (`SELECT`, `FROM`, `AS`, `DISTINCT`). Returns `Vec<Token>`.

**Step 16. `gate1/lib.rs`** — `extract_columns(sql: &str) -> ColumnExtraction`:

```rust
pub enum ColumnExtraction {
    Wildcard,                                 // SELECT *
    Columns(Vec<ExtractedColumn>),            // explicit list
    Unknown,                                  // can't parse confidently
}
pub struct ExtractedColumn {
    pub output_name: String,    // alias if present, else stripped column
    pub original: String,       // pre-alias column name (used as type label)
}
```

Walk tokens between `SELECT` and `FROM`, splitting on commas at the top paren level. For each entry: detect `AS <alias>` or trailing identifier-as-alias; strip schema/table qualifiers (`u.email` → `email`); ignore function calls (`COUNT(*)` → not a column).

**Step 17. `gate1::build_plan(extraction, sql_action, wildcard_policy, denylist) -> RedactPlan`** — applies the action table from FR-3.

**Step 18. Tests** — Golden SQL strings: simple SELECT, aliases (`AS contact`, bare-identifier alias), qualified columns (`u.email`, `users.email`), `SELECT *`, `SELECT DISTINCT`, function calls (`LOWER(email)`), CTEs (best-effort — document as unsupported), subqueries in SELECT list (best-effort), comments inline. Each test pins the extracted column list.

**Exit criterion:** All golden cases pass. Document explicit limitations in a comment block at the top of `lib.rs`.

---

## Milestone 4 — `redact run` (the worker)

Goal: end-to-end pipeline — spawn subprocess, capture stdout, run both gates, emit JSON.

**Step 19. `redact/main.rs`** — `clap` derive setup with all subcommands: `Run`, `Hook`, `Init`, `Config`, `List`, `Validate`, `Version`. Dispatch to module handlers. Harness gating is a single `is_agent_harness()` check at the top of `Init` and interactive `Config` handlers.

**Step 20. `redact/run.rs`** — `run(args: Vec<String>)`:

1. Load config.
2. Inspect `args[0]` (basename) → look up `tools[name].sql_arg`.
3. If `sql_arg` is set, scan `args` for `--sql VALUE` or `--sql=VALUE` (or whatever the configured flag is). Run `gate1::build_plan(...)` with the extracted SQL → `RedactPlan`.
4. If `plan.rejected`, emit error JSON, exit 1.
5. Spawn subprocess with `args[0]` and `args[1..]`, inheriting parent env, capturing stdout. Wait.
6. If subprocess exit ≠ 0, forward stdout (it may already be an error JSON or arbitrary text) and propagate exit code.
7. Parse stdout as JSON. If parse fails, forward unchanged (the tool may have emitted non-JSON for a reason).
8. Run Gate 2 with the plan.
9. Print compact JSON to stdout, exit 0.

**Step 21. Subprocess plumbing** — use `std::process::Command`. Stream stderr through unchanged (do not buffer). Do not add a redact-side timeout in v1; the underlying tool has its own.

**Step 22. Tests** — Integration tests that wire `redact run` against a fake-tool binary (a tiny shell script that emits known JSON for known SQL), assert end-to-end behavior on: tkpsql-shape, mysql-shape, error pass-through, non-JSON output pass-through, non-zero exit code propagation.

**Exit criterion:** `redact run -- ./fake-tkpsql --sql "SELECT email FROM users"` produces the expected redacted JSON. `cargo test -p redact` integration tests pass.

---

## Milestone 5 — Hook surface (`hook` + `init` + `config` + `list` + `validate`)

Goal: the install flow and the harness-facing surface.

**Step 23. `redact/hook.rs`** — `hook()`:

1. Read full stdin (the Bash command line as a single string).
2. `shell_words::split` to get tokens. If parse fails, emit unchanged + exit 0.
3. Take basename of `tokens[0]` (strip leading paths).
4. Load config. If basename not in `tools:`, write input verbatim to stdout, exit 0.
5. If tokens start with `redact run`, write input verbatim (loop avoidance), exit 0.
6. Build rewrite: `redact run -- <original command>` (preserve quoting using `shell_words::join`). Write to stdout, exit 0.

**Step 24. Performance discipline** — config load on each hook invocation must be ≤ 5ms. Measure with `criterion` if needed; if too slow, add a parsed-config cache keyed on file mtime in `~/.cache/redact/config.bin`. Defer the cache unless benchmarks show the need.

**Step 25. `redact/init.rs`** — `init(harness: Harness)`:

1. Validate harness is `claude-code` (the only supported value in v1).
2. Resolve target path (`~/.claude/settings.json`).
3. Read existing JSON or create `{}`.
4. Idempotently insert into `hooks.PreToolUse` an entry: `{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "redact hook" }] }`. If an entry with this exact `command` already exists, skip; print "already installed". If a different `redact hook` variant exists, replace. Never duplicate.
5. Write atomically (write to tempfile, rename).
6. Print success + next-step hint: "Run `redact config` to define which tools to intercept."

**Step 26. `redact/config_cmd.rs`** — `config(args)`:

- `--path`: resolve and print the config path. Exit 0.
- `--print`: load the file and print its raw contents to stdout. Exit 0.
- `--init-only`: if file missing, write starter from `starter.rs` (creating parent dir, logging the creation). Exit 0.
- No flags (default): same as `--init-only` if missing; then resolve `$VISUAL` → `$EDITOR` → `vi`, spawn it with the config path, wait for editor to exit. Inherit terminal stdio.

**Step 27. `redact/starter.rs`** — embedded starter config string with comments and the four built-in tools (`tkpsql`, `tkdbr`, `mysql`, `psql`). All commented except `tkpsql`/`tkdbr` to give a sensible default for the toolkit-companion case.

**Step 28. `redact/list.rs`** — Load config, print `tools:` entries: name + `sql_arg` value, two columns. For each tool, append "(raw client — credentials reachable to AI)" if it's in the raw-client set; "(toolkit-managed)" if it's `tkpsql`/`tkdbr`.

**Step 29. `redact/validate.rs`** — Load config, compile every regex (catching errors), warn on:

- Raw clients in `tools:` (soft warning, exit 0).
- Custom regex with no `confidence` field (use default, warn).
- Any `confidence > 1.0` or `< 0.0` (error, exit 1).
- Pattern key collides with a built-in but has different semantics (info).

Exit 0 if all checks pass with at most warnings; exit 1 on errors.

**Step 30. Tests** — Hook: 12+ cases — passthrough, intercept, loop avoidance, malformed input, raw-client (`mysql ...`), command with quoting (`tkpsql --sql "SELECT 'a b'"`). Init: idempotency (run twice, assert no duplicate), upgrade (different command string gets replaced), file creation when missing. Config: starter creation, $EDITOR fallback chain (test by setting envs to `/usr/bin/true`).

**Exit criterion:** End-to-end smoke test: clean `~/.config`, run `redact init && redact config --init-only`, then simulate a Claude Code Bash hook call by piping a tkpsql command into `redact hook`, observe correct rewrite. Run the rewrite under a fake-tool shim, observe correct redaction.

---

## Milestone 6 — Polish & ship

**Step 31.** README with quickstart (3 commands: install, init, config).

**Step 32.** Error message audit: every user-facing error names the actionable next step.

**Step 33.** Performance check: NFR-1 says <100ms for 1000 rows × 50 cols. Benchmark with `criterion` against a synthetic payload; if blown, profile.

**Step 34.** Manual smoke test against actual `tkpsql` and `mysql --json`. The fake-tool tests are necessary but not sufficient — real tools have surprises (mysql's NULL representation, postgres array syntax, etc.).

**Step 35.** Tag v0.1.0.

---

## Critical files to write or carefully shape

| File | Why critical |
|---|---|
| `common/redactor.rs` | The load-bearing safety net. Bugs here = PII leaks. |
| `gate1/lib.rs` | Best-effort SQL parsing. Wrong here = false-negative on Gate 1, but Gate 2 catches it. Lower stakes than redactor but worth golden-test coverage. |
| `redact/hook.rs` | Runs on every Bash command — perf and correctness both matter. |
| `redact/init.rs` | Touches the user's harness settings JSON. Idempotency and atomic writes are mandatory. |
| `redact/run.rs` | Spawns subprocesses, handles their stdio. The integration glue — most cross-component bugs live here. |

---

## Risks and mitigations

1. **JSON parse failure on legitimate non-JSON output.** Some tools may emit a banner line before JSON. Mitigation: forward unparseable stdout unchanged; document this as known. If it bites real users, add a `json_starts_with: "{"` heuristic.

2. **`shell-words` parse mismatch with Bash.** `shell-words` doesn't perfectly emulate Bash (e.g., `$(...)` expansion). For the hook's simple matching purpose this is fine — we only need argv[0]. But document the limitation.

3. **Hook performance regression.** If config grows or someone adds 50 patterns, the hook gets slow. Mitigation: benchmark before shipping; add the mtime cache if needed.

4. **`tkpsql`/`tkdbr` output shape changing.** External dependency. Mitigation: shape detection (Milestone 2) is generic enough that adding a new shape is one match arm.

5. **Claude Code's hook contract changing.** External dependency. Mitigation: document the contract version in `init.rs` so we can detect drift.

---

## What we're explicitly not doing in v1

- ML-based PII classification
- Streaming / chunked output
- Multi-harness support (Cursor, Gemini CLI, etc.)
- Per-tool PII overrides
- Audit logging
- Schema lookup for `SELECT *` resolution
- Encrypted config
- IPv6 detection, non-Latin name detection

These are listed in requirements.md "Out of Scope" / Open Questions and are deferred behind v1.

---

## Effort estimate

5–7 working days for one engineer comfortable with Rust. Milestones 1–4 are roughly half the work (the core); Milestones 5–6 are the other half (UX surface, polish, real-world testing).
