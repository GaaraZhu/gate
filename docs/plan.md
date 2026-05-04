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

## Milestone 7 — Close the non-JSON output gap

**Motivation:** Gate 2 currently requires JSON stdout. The shipped design relies on a per-tool `json_tool` config field (see `crates/redact/src/config.rs:22` and `crates/redact/src/hook.rs:57`): when the AI types `psql -c "..."`, the hook rewrites the spawn target to a side binary like `psql-json` and that wrapper is what produces JSON. The demo works because `psql-json` is installed on the demo host. The mechanism breaks silently when the wrapper is missing — inside a Docker container, on a fresh laptop, in CI — because (a) `psql`/`mysql` have no native `--json` output flag, (b) nothing pre-flights the wrapper's existence, and (c) on parse failure today's `redact run` forwards stdout unchanged. Net result: a silent PII leak whenever the wrapper isn't on PATH.

The right answer is to make `redact run` produce JSON itself by **rewriting the SQL** before spawning the subprocess, using the JSON-construction functions every modern relational DB ships with (`row_to_json` / `json_agg` in Postgres, `JSON_OBJECT` / `JSON_ARRAYAGG` in MySQL). When rewrite succeeds we keep full Gate 2 protection (column-aware, forced-column path applies). When rewrite is not safe — multi-statement input, `\` meta-commands, `COPY`, `EXPLAIN`, side-effecting queries — we fail closed instead of leaking. This makes `redact` self-contained: no side wrapper needs to exist, no PATH dependency in the container.

`json_tool` becomes deprecated by `rewrite_sql`. They solve the same problem; `rewrite_sql` does it without requiring an extra binary on PATH. We keep `json_tool` working for one release for backwards compatibility, then remove it.

This milestone has three parts: (A) fail closed on non-JSON output and pre-flight `json_tool` existence (the safety net), (B) SQL rewrite for raw clients (the primary path), (C) deprecate `json_tool` in favor of `rewrite_sql`.

### Part A — Fail closed on non-JSON + pre-flight `json_tool` (must-have, ships first)

**Step 36. `redact run` fails closed when stdout is not JSON.** Today: parse failure → forward unchanged. New behavior: parse failure → emit `{"error": "tool stdout was not JSON; refusing to forward unredacted output. Configure rewrite_sql for this tool, or install the json_tool wrapper."}` and exit 1. Update integration tests for the new failure mode.

**Step 37. `redact run` pre-flights `json_tool` before spawn.** When a tool's `json_tool` is configured, `redact run` resolves it on PATH (the same lookup the OS will do at exec time) before spawning. If not found, emit `{"error": "configured json_tool '<name>' not found on PATH; cannot redact output. Install the wrapper or configure rewrite_sql instead."}` and exit 1. Without this check the OS exec error bubbles up as a generic failure and the AI gets confused / retries the bare `psql` command; the explicit error is both safer and more debuggable. Add a unit test using a tempdir-controlled PATH.

**Step 38. `validate` warns on raw clients without rewrite or wrapper configured.** When a tool entry has a basename in the raw-client set (`psql`, `mysql`, `sqlite3`, `mongosh`, `mysqlsh`) and neither `rewrite_sql` (Step 39) nor an existing `json_tool` is configured, emit a hard warning naming the tool and pointing at the rewrite option. When `json_tool` *is* configured, `validate` also resolves it on PATH and warns if missing — same logic as the runtime pre-flight, surfaced earlier. Soft-warn-on-raw-client (existing FR) is upgraded from "credentials reachable" to also cover output-format risk.

**Exit criterion for Part A:** Three behaviors verified by tests: (i) a `psql -c "SELECT email FROM users"` invocation with no `json_tool` and no `rewrite_sql` returns the Step 36 error JSON; (ii) the same invocation with `json_tool: psql-json` and `psql-json` not on PATH returns the Step 37 error JSON; (iii) `redact validate` warns on both conditions. This part is independently shippable — Part B builds on top.

### Part B — SQL rewrite for raw clients (primary path)

**Step 39. Per-tool `rewrite_sql` config.** Add to `ToolConfig`:

```rust
pub enum RewriteDialect { Postgres, Mysql }
pub struct ToolConfig {
    pub sql_arg: Option<String>,
    pub json_tool: Option<String>,              // deprecated; see Part C
    pub rewrite_sql: Option<RewriteDialect>,    // None = no rewrite
    pub rewrite_extra_args: Vec<String>,         // e.g. ["-t", "-A"] for psql
}
```

When `rewrite_sql` is set, `redact run` rewrites the SQL string in the configured `sql_arg` and prepends `rewrite_extra_args` to the spawned argv. If both `rewrite_sql` and `json_tool` are set on the same entry, `rewrite_sql` wins and `validate` warns about the redundant `json_tool`.

**Step 40. `gate1::rewrite::wrap_select(sql, dialect, plan) -> RewriteResult`.** Returns one of:

```rust
pub enum RewriteResult {
    Rewritten(String),       // safe to send to the DB
    Skip(String),            // reason — fall through to fail-closed
}
```

Rewrite rules:

- **Postgres:** `<sql>` → `SELECT coalesce(json_agg(row_to_json(_r)), '[]'::json) FROM (<sql>) _r`. Combined with `psql -t -A`, output is a single line containing a JSON array.
- **MySQL:** `<sql>` → `SELECT JSON_ARRAYAGG(JSON_OBJECT(<key/val pairs from Gate 1's extracted columns>)) FROM (<sql>) _r`. Requires Gate 1 to have an explicit column list — `SELECT *` and `Unknown` extractions skip the rewrite. Combined with `mysql -N -B`.

Skip conditions (return `Skip` with reason):

- Multiple statements separated by `;` (after stripping trailing whitespace/comments).
- Leading `\` meta-command (psql).
- Top-level keyword is not `SELECT` / `WITH` (so `COPY`, `EXPLAIN`, `SHOW`, DML, DDL all skip).
- For MySQL: Gate 1 column extraction is `Wildcard` or `Unknown`.
- SQL already wraps in `json_agg` / `JSON_ARRAYAGG` (don't double-wrap — detect and pass through).

**Step 41. `redact run` dispatches on `rewrite_sql`.** New flow when `rewrite_sql` is set:

1. Load config, find tool entry, find SQL via `sql_arg`.
2. Run Gate 1 column extraction → `RedactPlan` (unchanged).
3. Call `wrap_select(sql, dialect, plan)`.
4. If `Rewritten(new_sql)`: substitute back into argv at the `sql_arg` position; prepend `rewrite_extra_args` to the rest of argv.
5. If `Skip(reason)`: append the reason to `plan.warnings` and proceed without rewrite. The fail-closed check from Step 36 catches the resulting non-JSON output.
6. Spawn subprocess, capture stdout.
7. Parse stdout. For Postgres: stdout is a JSON array (or `[]`); wrap as `{"rows": <array>}` for the existing shape pipeline. For MySQL: same — `JSON_ARRAYAGG` returns a JSON array.
8. Run Gate 2 with the plan as today.

**Step 42. Output unwrapping and shape consistency.** The rewritten output is always a JSON array. Reuse Milestone 2's array-shape handling: wrap as `{"rows": [...], "_redact_summary": ...}` when `include_summary: true`, otherwise return the bare array. The AI sees the same shape it would see from `tkpsql`, so prompt expectations stay consistent.

**Step 43. Error and edge-case handling.**

- **DB error from the rewritten query.** If the subprocess exits non-zero, forward stdout unchanged and propagate exit. Add a warning to `_redact_summary.warnings` only if we successfully attached a summary (i.e., output parsed); for non-zero-exit + non-JSON output, just propagate.
- **DB rewrites the column names** (Postgres lowercases unquoted identifiers in `row_to_json`; aliases survive). Gate 1 already lowercases keys for forced-column matching, so this aligns. Add an explicit test that `SELECT email AS Contact FROM users` produces a key `contact` after rewrite and that Gate 1's plan still matches.
- **Empty result set.** Postgres' `json_agg` returns `NULL` on empty input; the `coalesce(..., '[]'::json)` wrap above handles it.
- **Whitespace / trailing semicolon in user SQL.** Strip trailing `;` and surrounding whitespace before wrapping; otherwise the subquery is a syntax error.

**Step 44. Tests.**

- Unit: `wrap_select` golden cases — simple SELECT, SELECT with WHERE, JOIN, CTE (`WITH ... SELECT`), aliases, qualified columns, trailing semicolon, leading whitespace.
- Unit: `wrap_select` skip cases — multi-statement, `\d users`, `COPY ... TO STDOUT`, `EXPLAIN SELECT ...`, `INSERT`, `UPDATE`, already-wrapped query.
- Integration: fake `psql` shim that echoes the SQL it received and emits a synthetic JSON array. Assert the rewritten SQL contains `json_agg(row_to_json(_r))` and the args contain `-t -A`. Assert the redacted output preserves `rows` shape.
- Integration: same for MySQL with `JSON_ARRAYAGG` + `-N -B`.
- Integration: confirm Part A's fail-closed still triggers when rewrite is `Skip`-ped (e.g. `psql -c "\d users"` returns the error JSON, not aligned text).
- Negative: `rewrite_sql: None` (default for `tkpsql`/`tkdbr`) is unaffected.

**Exit criterion for Part B:** A `psql -c "SELECT email, ssn FROM users"` invocation through `redact run` (with `psql` configured per the new README recipe) returns the same shape as `tkpsql` would — `{"rows": [{"email": "[PII:email]", "ssn": "[PII:ssn]"}], "_redact_summary": {...}}` — using only stock `psql`, no shim binary required. `psql -c "\d users"` continues to fail closed via Part A. Same for `mysql -e`.

### Part C — Deprecate `json_tool` in favor of `rewrite_sql`

**Step 45. Mark `json_tool` deprecated in code.** Keep the field parsing and the hook rewrite path working unchanged — backwards compatibility for one release. On config load, if any tool has `json_tool` set, log a one-line deprecation notice on stderr (not stdout — must not pollute hook output): `redact: json_tool is deprecated, use rewrite_sql instead. See docs.` `validate` surfaces the same notice as a warning.

**Step 46. Migrate first-party config and docs.** Update `redact/starter.rs` to use `rewrite_sql` for the `psql`/`mysql` entries (no `json_tool`). Update `README.md` to remove the `json_tool: psql-json` example and replace it with the rewrite recipe. Update `docs/design.md` to describe both fields with `rewrite_sql` as the recommended path and `json_tool` as deprecated. Add a one-liner migration note: "If you previously configured `json_tool: psql-json`, replace with `rewrite_sql: postgres` and `rewrite_extra_args: [\"-t\", \"-A\"]` and uninstall the wrapper."

**Step 47. Plan removal.** Note in `docs/plan.md` (this milestone) that `json_tool` will be removed in v0.3.0 — but **do not remove it now**. Removing in the same release that introduces the deprecation breaks every existing user.

**Step 48. Docs.**

- `docs/design.md`: add a "SQL Rewrite" subsection under the Two-Gate Model. Document the rewrite templates per dialect, the skip conditions, and the trade-off (rewrite changes what the DB sees — error messages and `EXPLAIN` plans will reference the wrapped query). Update the Call Chain ASCII diagram to show the rewrite step between Gate 1 and subprocess spawn. Mark `json_tool` deprecated and link to the migration note.
- `README.md`: replace the "Raw clients" section's `json_tool` examples with:
  ```yaml
  tools:
    psql:
      sql_arg: "-c"
      rewrite_sql: postgres
      rewrite_extra_args: ["-t", "-A"]
    mysql:
      sql_arg: "-e"
      rewrite_sql: mysql
      rewrite_extra_args: ["-N", "-B"]
  ```
  Explain the trade-off in one paragraph: the DB sees a wrapped query; non-SELECT statements (`\d`, `COPY`, DML) are not rewritten and fail closed.
- `CLAUDE.md`: update Non-negotiables — failing closed on non-JSON output is a non-negotiable; `rewrite_sql` is the supported path for raw clients; `json_tool` remains for backwards compatibility but is deprecated.

**Exit criterion for Part C:** Existing configs using `json_tool` still work end-to-end (regression test); deprecation warning appears on stderr exactly once per `redact run` and on `redact validate`; starter config and README no longer mention `json_tool`; migration note is discoverable from both the README and `docs/design.md`.

### Risks specific to Milestone 7

1. **SQL the rewriter doesn't recognize as safe.** Rare dialect features, vendor extensions, query hints. Mitigation: when in doubt, `Skip` and let Part A fail closed — degraded UX but never a leak.
2. **Error messages reference the wrapped query.** A syntax error from the user's inner SQL surfaces with `... in subquery _r` context. Document this; it's a UX paper cut, not a correctness bug.
3. **`row_to_json` column lowercasing in Postgres.** Unquoted identifiers come back lowercase. Gate 1 already normalizes to lowercase, so forced-column matching works. Add a regression test pinning this contract.
4. **MySQL `JSON_OBJECT` requires explicit columns.** `SELECT *` cannot be rewritten without a schema lookup. Mitigation: skip; Part A fails closed; document in README that `SELECT *` against MySQL via raw `mysql` is unsupported (use `tkdbr` or list columns).
5. **Performance of rewriting on the DB side.** `json_agg` over a million rows builds a single big JSON value in DB memory. Mitigation: this is the same risk the AI would hit if it wrote the JSON wrapper itself — out of scope for redact to mitigate. Document.
6. **A query that already returns JSON gets double-wrapped.** Detect single-column SELECT whose expression starts with `json_`/`JSON_` and skip; add tests.
7. **Deprecation noise breaks the hook.** Logs to stdout would corrupt the rewritten command. Strict rule: deprecation goes to stderr only. Test pins this.

### Effort estimate

3–3.5 working days. Part A is ~one day (fail-closed branch + `json_tool` PATH pre-flight + validate updates + tests). Part B is ~1.5 days (`gate1::rewrite` module ~150 lines, dispatch wiring in `run.rs`, two dialect templates, ~20 unit tests, two integration shims). Part C is ~half a day (deprecation notice plumbing, starter/README/design migration, regression test).

---

## Milestone 8 — GitHub Copilot CLI support

> **Status: DEFERRED to a future release.** The full spec below is retained for when we revisit. Reason for deferral: Copilot CLI's `preToolUse` hook contract only supports deny-with-suggestion, not transparent rewrite, so the integration is *advisory* — strictly safer than no hook, but the AI could ignore the suggestion. We're waiting for either (a) Copilot CLI to gain an `updatedInput` equivalent, or (b) clear user demand that justifies shipping the advisory-only mode. Milestone 9 (opencode) ships first because opencode's plugin contract supports enforcing rewrite. When Copilot CLI is picked up again, renumber as needed and re-validate the schema against the current `https://docs.github.com/en/copilot/reference/hooks-configuration` (it may have drifted).

**Motivation.** Today `redact hook` only understands Claude Code's snake_case PreToolUse JSON shape and only emits `updatedInput`-style transparent rewrites. Users running GitHub Copilot CLI get no protection — Copilot CLI ships its own PreToolUse hook contract with a different wire format (camelCase `toolName` / `toolArgs`-as-JSON-string) and, critically, does **not** support `updatedInput`. The fallback that works on Copilot CLI is **deny-with-suggestion**: the hook returns `permissionDecision: "deny"` with a reason that names the safe replacement, and the AI retries with that command. Pattern lifted directly from rtk's `src/hooks/hook_cmd.rs::run_copilot` (dual format detection + dual response).

This milestone adds Copilot CLI support without disturbing the Claude Code path: same binary, same decision pipeline, format detection at the boundary, dual response writers. It also adds `redact init --harness copilot-cli` to write the project-scoped hook config and an instructions snippet that materially raises Copilot's compliance with the deny-and-retry suggestion. VS Code Copilot Chat uses the same snake_case format as Claude Code, so it works automatically once the user wires the hook in their VS Code settings — no extra `init` mode needed in v1.

**Non-goal:** Cursor, Gemini CLI, OpenCode (still deferred). And a v1 Copilot CLI integration is **advisory**, not enforcing — see requirements.md "Enforcement strength varies by harness". This is acceptable because it strictly improves on the unhooked baseline (the original PII-leaking command is denied) and pairs the runtime hook with an `.github/copilot-instructions.md` block telling Copilot to honour the suggestion.

### Part A — Format detection and dual response in `redact hook`

**Step 49. Introduce `HookFormat` at the top of `process()` in `crates/redact/src/hook.rs`.**

```rust
enum HookFormat {
    ClaudeOrVsCode,      // snake_case tool_name / tool_input.command
    CopilotCli,          // camelCase toolName / toolArgs (JSON-encoded string)
}
fn detect_format(v: &serde_json::Value) -> Option<HookFormat>;
```

Detection rules (mirroring rtk):

- `tool_name` present (snake_case) → `ClaudeOrVsCode`. Accept `tool_name` values `Bash` or `bash`. Read the command from `tool_input.command`.
- `toolName` present (camelCase) → `CopilotCli`. Accept `toolName` value `bash`. Parse `toolArgs` as a JSON string (it's encoded as a string by Copilot CLI), then read `command` from the resulting object.
- Neither → return `None` and passthrough.

Reject anything that isn't a Bash invocation early — both formats may carry other tool names (`editFiles`, `runTerminalCommand`, etc.); `runTerminalCommand` from VS Code can be opportunistically accepted too if testing shows it appears in practice (rtk does).

**Step 50. Refactor `process()` so the decision pipeline is shape-agnostic.**

The existing token-walking + tool-match + json_tool rewrite + loop-avoidance logic does not change. Only the input extraction and output emission are branched. Suggested shape:

```rust
fn process(stdin: &str, config: &Config) -> Option<String> {
    if !config.enabled || is_disabled_by_env() { return None; }
    let v: Value = serde_json::from_str(stdin).ok()?;
    let format = detect_format(&v)?;
    let original_command = extract_command(&v, format)?;
    let new_command = rewrite_command(&original_command, config)?;  // existing logic
    Some(emit_response(format, new_command))
}
```

`emit_response` produces the existing `hookSpecificOutput.updatedInput` JSON for `ClaudeOrVsCode`, and the new `permissionDecision: "deny"` + reason JSON for `CopilotCli`.

**Step 51. Copilot CLI deny reason text.** Format: `PII safety: run \`<new_command>\` instead`. The leading `PII safety:` substring is the anchor that the `copilot-instructions.md` snippet (Step 54) tells Copilot to recognise; do not change this prefix without updating the instructions.

**Step 52. Tests.** Extend `crates/redact/src/hook.rs` `mod tests` with a `mod copilot_cli` block covering:

- detect_format: snake_case / camelCase / unknown / `tool_name` non-Bash / `toolName` non-bash / `toolArgs` malformed JSON.
- Intercept paths produce the right shape per format (assert `permissionDecision == "deny"` and reason contains `redact run --` for Copilot, `permissionDecision == "allow"` and `updatedInput.command` starts with `redact run --` for Claude).
- Passthrough conditions (config disabled, env-disabled, loop avoidance, tool not in config, malformed JSON) all return `None` regardless of format.
- Same payload sent in both formats produces semantically equivalent rewrites of the same command.

### Part B — `redact init --harness copilot-cli`

**Step 53. CLI surface.** Extend the `--harness` clap value enum to include `copilot-cli`. Dispatch in `init.rs::run` to a new `init_copilot::run()`. Keep the existing `is_agent_harness()` gate (init is blocked inside any harness).

**Step 54. `init_copilot::run` writes two files in the current working directory:**

1. `./.github/hooks/hooks.json` — project-scoped hook config (Copilot CLI loads hooks from the directory it launches in; the GitHub-recommended layout is `.github/hooks/hooks.json`). Schema follows [GitHub's hooks reference](https://docs.github.com/en/copilot/reference/hooks-configuration):
   ```json
   {
     "version": 1,
     "hooks": {
       "preToolUse": [
         {
           "type": "command",
           "bash": "redact hook",
           "cwd": ".",
           "timeoutSec": 5
         }
       ]
     }
   }
   ```
   Note the lowerCamelCase event name (`preToolUse`, not Claude's `PreToolUse`), the `bash` script field (not `command`), and `timeoutSec` (not `timeout`). Write atomically (tempfile + rename) into `.github/hooks/`, creating directories as needed.

   Idempotency rules:
   - If `hooks.json` does not exist → create it with the JSON above.
   - If it exists and already contains a `preToolUse` array entry whose `bash` field equals `"redact hook"` (or matches the `redact hook` basename pattern, mirroring `init.rs::is_redact_hook_variant`) → skip and print "already installed".
   - If it exists with other unrelated entries (the user has hand-curated hooks) → merge: append a new entry to `preToolUse` without disturbing other event arrays. Replace any pre-existing `redact hook` variant in place rather than duplicating.
   - The top-level `version: 1` field is preserved if present and inserted otherwise.

2. `.github/copilot-instructions.md` — upsert an anchor block:
   ```markdown
   <!-- redact:begin -->
   ## redact (PII safety hook)

   When a command is denied with a `permissionDecisionReason` starting with
   `PII safety:`, re-run with the exact command shown in backticks in that
   reason. Do not paraphrase or alter the suggested command — it routes the
   query through `redact run`, which redacts PII before returning results.
   <!-- redact:end -->
   ```
   Behaviour:
   - File missing → create with just this block.
   - File exists, anchor block present → replace contents between `<!-- redact:begin -->` and `<!-- redact:end -->` (preserves any user content outside the block).
   - File exists, anchor block absent → append `\n\n<block>\n` at the end.
   - Atomic write (tempfile + rename), same as `init.rs::write_atomic`.

**Step 55. Print the next-step hint.** After writing both files, print:
```
redact: GitHub Copilot CLI integration installed (project-scoped).
  Hook config:    .github/hooks/hooks.json   (preToolUse entry added)
  Instructions:   .github/copilot-instructions.md (redact block)

  Note: Copilot CLI uses deny-with-suggestion (the AI is asked to re-run
  with the redact-prefixed command). Compliance is improved by the
  copilot-instructions.md block above.

  Restart your Copilot CLI session to activate.
```

**Step 56. Tests.** In `crates/redact/src/init_copilot.rs` `mod tests`:

- Creates both files in a tempdir; content matches expected JSON / markdown exactly.
- Idempotent: running twice produces no duplicate anchor blocks; second run prints "already installed" or "no changes".
- Anchor-block upsert: pre-existing `copilot-instructions.md` with unrelated content keeps that content intact; the redact block is inserted/replaced cleanly.
- Atomic write: a write that fails mid-flight does not leave a corrupt file (assert no `.redact_tmp` file is left behind on success path).

### Part C — Docs and validate

**Step 57. `validate` reports installed harnesses.** When `redact validate` runs, additionally report which harness integrations appear installed (Claude: `~/.claude/settings.json` contains a `redact hook` PascalCase `PreToolUse` entry; Copilot: `./.github/hooks/hooks.json` exists in the cwd with a lowerCamelCase `preToolUse` `redact hook` entry). Read-only check, surfaced at the end of validate's existing output. No exit-code change.

**Step 58. README + docs migration.**

- `README.md` Installation section: split into two tabs (or two adjacent code blocks): "Claude Code" (`redact init`) and "GitHub Copilot CLI" (`redact init --harness copilot-cli`, run from the project root). Add a one-paragraph callout naming the deny-with-suggestion limitation.
- `README.md` Commands table: update `redact init` row to `redact init [--harness claude-code|copilot-cli]`.
- `docs/design.md` already updated in this milestone (per-harness installation surface, dual-format hook contract).
- `docs/requirements.md` already updated in this milestone (FR-2, FR-2a, "Enforcement strength varies by harness").
- `CLAUDE.md` Non-negotiables: add "Hook output format must match the detected input format. The Copilot CLI deny reason **must** start with the literal prefix `PII safety:` — the instructions snippet keys off this anchor."

### Risks and mitigations specific to Milestone 8

1. **Copilot may not honour the deny suggestion.** Mitigation: instructions snippet anchored on `PII safety:` raises compliance; document the advisory-not-enforcing limitation in three places (requirements, design, README); revisit if Copilot CLI gains an `updatedInput` equivalent.
2. **Format detection ambiguity.** A malformed message could carry both `tool_name` and `toolName`. Mitigation: `tool_name` wins (Claude/VS Code shape). Pin in tests.
3. **`toolArgs` JSON-string parsing failures.** Copilot CLI sends a string-encoded JSON object. If parsing fails, treat as passthrough (exit 0, no output). Test with malformed strings.
4. **Project-scoped hook leaks across repos.** `.github/hooks/hooks.json` lives in the user's repo; if the user enables redact in a public repo and pushes, downstream users get the hook. Mitigation: README install section calls this out and recommends `.gitignore`-ing the file when it shouldn't be committed.
6. **Schema drift from rtk's docs.** rtk's source uses Claude's PascalCase `PreToolUse` and a flat `command` field for its Copilot install — that does not match Copilot CLI's official schema (`preToolUse`, `bash`/`powershell`, `timeoutSec`, top-level `version: 1`). VS Code's hook host normalises both, but Copilot CLI does not — pin tests against the official schema, not against rtk's emitted JSON.
5. **Anchor-block parser corruption.** A file with one anchor but not the other could be misparsed. Mitigation: require both `<!-- redact:begin -->` and `<!-- redact:end -->`; if only one is present, treat as "no anchor" and append a fresh block (the user can clean up).

### Effort estimate

1.5 working days. Part A (format detection + dual emit + tests) is ~half a day — the existing decision pipeline doesn't change, only its bookends. Part B (init_copilot module + atomic upsert + tests) is ~half a day. Part C (validate signal + README/CLAUDE.md migration) is ~half a day.

### Exit criterion

End-to-end: in a project with `./.github/hooks/hooks.json` installed by `redact init --harness copilot-cli` (top-level `version: 1`, `hooks.preToolUse[]` containing the `bash: "redact hook"` + `timeoutSec: 5` entry), simulate a Copilot CLI hook invocation by piping the camelCase JSON shape into `redact hook`, observe a `permissionDecision: "deny"` response whose reason contains `redact run --` followed by the original command. The Claude Code golden cases continue to pass unchanged. `redact validate` reports both integration sites when both are installed.

---

## Milestone 9 — opencode support

**Motivation.** opencode (`sst/opencode`) is a popular open-source AI coding agent. Its plugin system exposes a `tool.execute.before(input, output)` hook whose `output.args` is mutable — mutating `output.args.command` for the bash tool propagates to the actual subprocess opencode runs. That gives us the same **enforcing** guarantee Claude Code provides (the AI cannot opt out of the rewrite), unlike Copilot CLI's deny-with-suggestion. The hook signature is canonical (verified in `sst/opencode:packages/plugin/src/index.ts`):

```typescript
"tool.execute.before"?: (
  input: { tool: string; sessionID: string; callID: string },
  output: { args: any },
) => Promise<void>
```

Bash-tool args shape (`packages/opencode/src/tool/shell/prompt.ts`): `{ command: string, timeout?, workdir?, description }`.

**Design.** No changes to the Rust `redact hook` contract. The opencode integration ships as a small TypeScript plugin file that:
1. Returns early if `input.tool !== "bash"`.
2. Synchronously spawns `redact hook`, piping snake_case JSON `{"tool_name":"Bash","tool_input":{"command":<cmd>}}` to stdin (the format `redact hook` already understands — Claude Code shape).
3. If stdout is non-empty, parses it as JSON, extracts `hookSpecificOutput.updatedInput.command`, and assigns it to `output.args.command`.
4. If stdout is empty, leaves `output.args.command` unchanged (passthrough — same convention as Claude Code).

This keeps the format-detection surface in `redact hook` to one shape (no opencode-specific Rust path), localises the opencode-specific glue in the plugin file, and reuses the existing `process()` decision pipeline verbatim.

**Non-goal:** Cursor, Gemini CLI, Aider — still deferred. The Copilot CLI work in Milestone 8 stays deferred.

### Part A — `redact init --harness opencode`

**Step 49. Extend the `--harness` clap value enum to include `opencode`.** Wire dispatch in `crates/redact/src/init.rs::run` to a new module `init_opencode::run()`. Keep the existing `is_agent_harness()` gate at the top of `init.rs::run` — opencode sets `OPENCODE`, so init is correctly blocked inside an opencode session (already verified in `crates/common/src/harness.rs:1`).

**Step 50. `init_opencode::run` writes a single plugin file.**

Default location: `~/.config/opencode/plugin/redact.ts` (global, mirroring the user-scope of `~/.claude/settings.json` for Claude Code). opencode loads plugins from this directory automatically; no settings-file edit is required.

Add a `--scope project|global` flag (default `global`) so users can opt into project-scoped install at `./.opencode/plugin/redact.ts` instead. Project scope is useful when the user doesn't want every opencode session globally redacted (e.g. they only want it for one repo); global scope matches Claude Code's behaviour and is the recommended default.

Plugin contents (sketch — pin exactly in code, target ~40 lines):

```typescript
// .opencode/plugin/redact.ts (or ~/.config/opencode/plugin/redact.ts)
// Generated by `redact init --harness opencode`. Safe to delete.
import { spawnSync } from "node:child_process"

export const RedactPlugin = async () => ({
  "tool.execute.before": async (
    input: { tool: string },
    output: { args: { command?: string } },
  ) => {
    if (input.tool !== "bash") return
    const cmd = output.args.command
    if (typeof cmd !== "string" || cmd.length === 0) return

    const payload = JSON.stringify({
      tool_name: "Bash",
      tool_input: { command: cmd },
    })
    const result = spawnSync("redact", ["hook"], {
      input: payload,
      encoding: "utf8",
      timeout: 5000,
    })
    if (result.status !== 0 || !result.stdout) return  // passthrough
    try {
      const parsed = JSON.parse(result.stdout)
      const updated = parsed?.hookSpecificOutput?.updatedInput?.command
      if (typeof updated === "string" && updated.length > 0) {
        output.args.command = updated
      }
    } catch {
      // malformed response — fail open to passthrough; never block the user
    }
  },
})
```

Atomic write (tempfile + rename, same helper as `init.rs::write_atomic`). Create parent directories as needed.

Idempotency rules:
- File missing → write fresh.
- File exists with byte-identical contents → skip; print `redact opencode plugin already installed at <path>`.
- File exists with a recognisable redact header (the first comment line above) → overwrite (treat as upgrade — matches `init.rs`'s "redact hook variant" replacement behaviour).
- File exists without the redact header → refuse to overwrite; print an error pointing the user to delete or rename the existing file. (Avoid clobbering a user-authored plugin that happens to share the filename.)

**Step 51. Print the next-step hint.** After the write, print:

```
redact: opencode integration installed.
  Plugin:   ~/.config/opencode/plugin/redact.ts        (global scope)
            (use --scope project for ./.opencode/plugin/redact.ts)

  Restart your opencode session to load the plugin.
  Run `redact config` to define which tools to intercept.
```

**Step 52. Tests** in `crates/redact/src/init_opencode.rs` `mod tests`:

- Tempdir-controlled `HOME` and project root: writes file at expected path, contents match the embedded template byte-for-byte.
- `--scope project` writes `./.opencode/plugin/redact.ts`; `--scope global` (default) writes the user-config path.
- Idempotent: running twice with the redact header yields one write + one "already installed" report; no duplicate file or trailing tempfile.
- Clobber guard: pre-existing `redact.ts` without the redact header causes an error and no write.
- The harness gate from `init.rs::run` still fires when `OPENCODE` is set (regression — guard-by-default also covers opencode).

### Part B — Hook reuse and end-to-end shim

**Step 53. No changes to `crates/redact/src/hook.rs`.** The opencode plugin sends snake_case JSON, so the existing `process()` path at `crates/redact/src/hook.rs:26` handles it identically to Claude Code. Add a comment at the top of `hook.rs` noting that the snake_case shape is the canonical input for both Claude Code (sent directly by the harness) and opencode (sent by the bundled plugin file).

**Step 54. Integration smoke test.** A new integration test in `crates/redact/tests/` that:
1. Reads the embedded plugin TS template (compile-time `include_str!`).
2. Asserts the plugin's payload format matches what `hook::process()` expects: parse the embedded TS, extract the JSON template from the `JSON.stringify(...)` call, and assert it round-trips through `process()` with a known intercept-eligible command.

This is a contract test between the JS template and the Rust hook — if either side drifts, the test fails. Cheaper than wiring an actual opencode runtime in CI.

### Part C — validate, config, docs

**Step 55. `validate` reports installed harness integrations.** Extend the existing `validate.rs` end-of-output section: detect Claude Code (`~/.claude/settings.json` with a `redact hook` `PreToolUse` entry) AND opencode (`~/.config/opencode/plugin/redact.ts` exists with the redact header, OR `./.opencode/plugin/redact.ts` exists in cwd). Read-only check, no exit-code change. (When Milestone 8 ships, the same surface absorbs the Copilot CLI check.)

**Step 56. Starter config unchanged.** opencode reuses every tool entry from `~/.config/redact/config.yaml` — there is no per-harness config split. Verify with a manual smoke test that `tkpsql` interception works the same way under opencode as under Claude Code.

**Step 57. Docs.**

- `README.md` Installation: promote opencode from "Roadmap" to a second supported-harness block alongside Claude Code. Note that the integration is enforcing (transparent rewrite via plugin mutation of `output.args.command`).
- `README.md` Commands table: update `redact init` row to `redact init [--harness claude-code|opencode] [--scope project|global]`.
- `docs/design.md` per-harness installation surface table: add an opencode row pointing at the plugin file path; update the "Single binary, dual contract" section to clarify opencode reuses the snake_case shape via the plugin glue (one Rust contract, two harnesses).
- `docs/requirements.md` FR-2 / FR-2a / FR-2c / Enforcement table: add opencode as Enforcing alongside Claude Code; explicitly call out the install model (plugin file vs settings entry).
- `CLAUDE.md` Non-negotiables: extend the "Hook output format" item to note that opencode also speaks the snake_case shape via its bundled plugin (no Rust-side format change).

### Risks specific to Milestone 9

1. **opencode plugin API changes.** The hook signature is documented but not stabilised on a versioning policy. Mitigation: pin a minimum opencode version in the README install block; add a one-line "tested with opencode vX.Y" note at the top of the embedded plugin template; the contract test (Step 54) catches drift between the plugin and `hook.rs`.

2. **`spawnSync` on every bash command adds latency.** Each bash tool call now forks a `redact` process. With config caching (already in place — see Milestone 5 Step 24's mtime cache discussion), single-digit ms is the target. Mitigation: add a benchmark in the contract test asserting the round-trip is <50ms p99 on a warm cache.

3. **Bun vs Node runtime differences in the plugin.** opencode runs on Bun. `node:child_process.spawnSync` works under Bun but `Bun.spawnSync` is the more idiomatic API. Choosing `node:child_process` keeps the plugin portable if opencode ever moves off Bun. Pin the choice in code with a comment.

4. **`--scope global` collides between opencode versions / projects.** A user installing the plugin globally then opening a different project still gets PII filtering. This is a feature — global means global. But document explicitly: users who want per-project scoping must use `--scope project`. Worth a one-paragraph callout in the README.

5. **Plugin file accidentally checked into a public repo (project scope).** Same risk Milestone 8 flagged for `.github/hooks/hooks.json`. Mitigation: same — README install section recommends `.gitignore`-ing `.opencode/plugin/redact.ts` if the repo is public.

6. **`output.args.command` mutation may be ignored if opencode's plugin contract changes.** Mitigation: smoke test the actual mutation against a real opencode binary at release time (not in CI — opencode is heavy to install). Document the contract dependency in `docs/design.md`.

### Effort estimate

1 working day. Part A (clap enum + `init_opencode.rs` + idempotent write + tests) is ~half a day. Part B (template wiring + contract test, no `hook.rs` changes) is a couple of hours. Part C (validate signal + README/design/requirements/CLAUDE.md migration) is ~quarter of a day.

### Exit criterion

End-to-end: with `~/.config/opencode/plugin/redact.ts` installed by `redact init --harness opencode`, start an opencode session, ask the AI to run `tkpsql query --sql "SELECT email FROM users"`, observe the bash tool actually executes `redact run -- tkpsql query --sql "..."` and the AI sees `[PII:email]` in the result. Claude Code golden cases continue to pass unchanged. `redact validate` reports both integration sites when both are installed.

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
