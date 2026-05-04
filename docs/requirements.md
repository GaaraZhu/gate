# Requirements: redact

## Problem Statement

AI coding agents querying production datastores can inadvertently exfiltrate PII (Personally Identifiable Information). Existing query tools (e.g. `tkpsql`, `tkdbr`) enforce write protection and credential hiding but perform no inspection of outbound data. A single `SELECT *` against a users table can expose SSNs, emails, phone numbers, and payment data directly into the AI's context window — and from there into logs, prompts, and model training pipelines.

`redact` is a standalone CLI that registers a PreToolUse hook in the agent harness, transparently routes the AI's query commands through a two-gate PII filter, and returns sanitized JSON. The AI runs its normal commands; the hook silently intercepts them. Humans and CI scripts running outside the harness are unaffected.

---

## Goals

- Prevent PII from appearing in query results returned to AI agents
- AI agents need no awareness of `redact` and cannot bypass it: install is a single agent-harness hook registration; the AI runs its normal query commands and the hook silently routes them through `redact`
- Support any CLI that takes SQL via a configurable flag and outputs JSON rows on stdout (output-format-agnostic, not security-stack-agnostic — see Security Model)
- Be configurable per-tool and per-deployment without code changes
- Produce output indistinguishable in shape from the underlying tool (so AI agent prompts need no adjustment)

## Non-Goals

- Replacing existing query tools (`tkpsql`, `tkdbr`) — `redact` wraps them
- Enforcing write protection or access control (that is toolkit's job)
- Managing or protecting database credentials (out of scope — see Security Model)
- Filtering output for human users or CI scripts (the only consumer is the AI agent; non-AI shells run tools normally without `redact`)
- Supporting agent harnesses without a pre-execution hook mechanism (v1 = Claude Code; others added incrementally)
- Scanning non-JSON output formats
- Real-time streaming query result scanning
- Network-level proxying (HTTP/socket); CLI subprocess model only

---

## Security Model

`redact` is one layer in a defense-in-depth stack for AI agents accessing datastores. Each layer addresses a different threat and can be adopted independently based on the user's needs.

| Layer | Protects against | Required when |
|---|---|---|
| **Agent harness sandbox** (e.g. Claude Code permissions, executable allowlists) | AI bypassing wrappers by invoking raw clients directly | Always — without it, the other layers are bypassable |
| **toolkit** (`tkpsql`, `tkdbr`) | Write operations; credential exposure in argv, env, or files reachable by the AI | Credentials are sensitive, or write access exists |
| **redact** | PII leaking through query results | Tables contain PII the AI must not see |

### Deployment Models

| Situation | Recommended stack |
|---|---|
| Production DB, sensitive credentials, contains PII | harness + toolkit + redact |
| Production DB with read-only credentials, contains PII | harness + redact |
| Production DB, sensitive credentials, no PII risk | harness + toolkit |
| Local sqlite, throwaway data, no PII | harness only |
| Public/anonymized dataset | harness only |

### Scope boundaries

- `redact` filters PII from query *output* only. It does not protect credentials.
- Any credential reachable by the wrapped tool (e.g. `~/.my.cnf`, `~/.pgpass`, env vars) is also reachable by the AI agent. `redact` cannot mitigate this.
- For credential protection, wrap a toolkit-managed client (`tkpsql`/`tkdbr`) — toolkit injects credentials into the subprocess env at spawn time from an encrypted store, never exposing them to the AI.
- For raw-client wrapping (e.g. `mysql`), users accept that credentials in any location the tool reads are reachable by the AI. This is acceptable when the credential is low-sensitivity (read-only DB user, local dev DB) but is documented as a security trade-off, not a recommended default.
- `redact validate` emits a soft warning when a configured tool entry is a raw client (`mysql`, `psql`, `mongosh`, `sqlite3`, …) rather than a toolkit wrapper, so users are informed of the gap.

### Enforcement strength varies by harness

`redact`'s safety guarantee depends on whether the harness's hook contract supports transparent rewrite (`updatedInput`):

| Harness | Mechanism | Guarantee | Status |
|---|---|---|---|
| Claude Code | `updatedInput` rewrite | **Enforcing** — the AI cannot opt out; the rewritten command is what the harness runs | Shipped |
| opencode | `tool.execute.before` plugin mutates `output.args.command` | **Enforcing** — same guarantee as Claude Code; the plugin rewrites the bash tool's args before the subprocess spawns | Planned (Milestone 8) |
| VS Code Copilot Chat | `updatedInput` rewrite | **Enforcing** | Compatible by reusing the snake_case shape; user wires the hook in their VS Code settings (no `redact init` mode in v1) |
| GitHub Copilot CLI | `permissionDecision: deny` + suggestion | **Advisory** — Copilot CLI's API does not support rewrite; the AI is denied and asked to retry with the suggested command. It may comply, abandon the query, or attempt a workaround | **Deferred** (Milestone 9 spec retained in plan.md) |

Copilot CLI is deferred to a future release. Reason: deny-with-suggestion is materially weaker than transparent rewrite — strictly safer than no hook, but the AI could ignore the suggestion. We're holding the integration until either Copilot CLI gains an `updatedInput`-equivalent (in which case `redact hook` should switch to that path automatically) or user demand justifies shipping the advisory-only mode. Until then, the documented enforcement story is uniformly "enforcing".

---

## Functional Requirements

### FR-1: Configuration

- Config file at `~/.config/redact/config.yaml` (overridable via `REDACT_CONFIG` env var)
- Top-level structure:
  - `tools:` — map keyed by executable basename (`tkpsql`, `tkdbr`, `mysql`, `psql`, …). Each entry specifies `sql_arg` — the flag the tool uses to receive SQL (default `--sql`; `null` disables Gate 1 for that tool).
  - `pii:` — global PII policy: column-name denylist, regex patterns, confidence threshold, action/wildcard policy, redaction template, summary toggle.
- Connection identity (e.g. `--connection dev`) is **not** modeled in `redact`'s config. It travels through the AI's command line and is captured only as provenance metadata for `_redact_summary`.
- The config contains **no credentials**. Credential values, password flags with literal values, or env var interpolation are forbidden. Credentials are the wrapped tool's responsibility (see Security Model).

### FR-2: Hook installation (`redact init`)

- `redact init [--harness <name>]` writes the harness-specific hook configuration so `redact hook` is invoked as a PreToolUse callback. Default `--harness` value is `claude-code`.
- Supported harnesses:
  - `claude-code` — writes a `PreToolUse` entry into `~/.claude/settings.json` registering `redact hook` against the `Bash` matcher. **Shipped.**
  - `opencode` — writes a TypeScript plugin file at `~/.config/opencode/plugin/redact.ts` (or `./.opencode/plugin/redact.ts` with `--scope project`). The plugin's `tool.execute.before` hook mutates `output.args.command` to the rewritten value, giving an enforcing guarantee equivalent to Claude Code. **Planned — Milestone 9.**
  - `copilot-cli` — writes a project-scoped `preToolUse` hook config and a `.github/copilot-instructions.md` snippet. **Deferred to a future release** (advisory enforcement only — see Security Model). Spec retained in `docs/plan.md` Milestone 9.
- Other harnesses (Cursor, Gemini CLI, Aider) remain out of scope.
- `redact init` is idempotent for every supported harness: re-running detects an existing hook entry and skips or upgrades it; never duplicates. For `opencode`, the plugin file is overwritten in place when its leading redact header is present (treated as upgrade) and refused when the header is absent (avoids clobbering a user-authored file with the same name).

### FR-2a: Hook execution (`redact hook`)

- Implements the calling harness's PreToolUse callback contract. Reads the about-to-run command from stdin as JSON, decides intercept vs. passthrough, and emits a harness-appropriate JSON response (or no output for passthrough).
- **Input format.** v1 supports a single on-the-wire shape — the snake_case PreToolUse JSON used by Claude Code and VS Code Copilot Chat: `{"tool_name":"Bash","tool_input":{"command":"..."}}`. Extracts `tool_input.command`. Any other shape: passthrough.
- **opencode reuses the same shape via a bundled plugin** (Milestone 8). The plugin file `redact init --harness opencode` writes runs inside opencode's TypeScript plugin runtime, formats the bash tool's args as snake_case JSON, pipes them to `redact hook`, then mutates `output.args.command` from the response. From `redact hook`'s point of view there is exactly one input shape; the per-harness translation lives in the plugin glue.
- **Decision logic** (independent of harness once the command string is extracted):
  1. Parse the command line; extract the configured tool token (basename).
  2. If the tool basename is not a key in `tools:`, passthrough.
  3. If the command is already prefixed with `redact run`, passthrough (loop avoidance).
  4. Otherwise, build the rewritten command string `redact run -- <original command>`.
- **Output format:** emit `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{"command":"redact run -- ..."}}}`. Both Claude Code (directly) and the opencode plugin (which extracts `hookSpecificOutput.updatedInput.command` and assigns it to `output.args.command`) consume this shape.
- **Copilot CLI's camelCase deny-with-suggestion shape is specced in `docs/plan.md` Milestone 9 but deferred** — see Security Model for why the advisory model is held back.
- **Passthrough path** writes nothing to stdout and exits 0; every harness treats no-output-from-hook as "run the original command unchanged".
- Must be fast on the passthrough path (single-digit ms) since it runs on every Bash command.

### FR-2b: Config management (`redact config`)

- `redact config` — if the config file does not exist, write a starter config (built-in tool examples + default PII policy, with comments) and create the parent directory if missing; then open the config in `$VISUAL` → `$EDITOR` → `vi` fallback chain.
- `redact config --path` — print the resolved config path and exit. Read-only; allowed in agent context.
- `redact config --print` — print the resolved config to stdout. Read-only; allowed in agent context.
- `redact config --init-only` — create a starter config if missing, but do not open an editor. Useful for non-interactive provisioning.

### FR-2c: CLI subcommand surface

`redact` exposes the following explicit subcommands:

| Command | Purpose | Blocked in agent harness? |
|---|---|---|
| `redact run -- <tool args...>` | Execute the wrapped tool with Gate 1 + Gate 2 (invoked by the hook, not by users) | No |
| `redact hook` | PreToolUse callback: rewrite or passthrough the incoming Bash command | No |
| `redact init [--harness claude-code\|opencode] [--scope project\|global]` | Register the PreToolUse hook in the harness settings (Claude Code: `~/.claude/settings.json`; opencode: TypeScript plugin file at the chosen scope). `copilot-cli` is deferred — see plan.md Milestone 8. | Yes |
| `redact config [--path \| --print \| --init-only]` | Manage `redact`'s own config file (interactive edit by default) | Yes (interactive); `--path`/`--print` allowed |
| `redact list` | Print configured tools and their `sql_arg` values | No (read-only) |
| `redact validate` | Load config, compile patterns, report errors and soft warnings (raw-client warning, overly broad patterns) | No |
| `redact version` | Print version | No |

### FR-3: Gate 1 — Pre-query SQL inspection

- Triggered when the underlying command includes a SQL flag matching the tool's configured `sql_arg`
- Parse the SQL statement to extract referenced column names
- Cross-check extracted column names against the configured PII column denylist (case-insensitive)
- If a denylisted column is found in a `SELECT`:
  - `action: warn` — proceed but annotate response with a warning
  - `action: redact` — proceed and redact that column's values in Gate 2
  - `action: reject` — return `{"error": "query targets PII column: <name>"}` and exit 1
- `SELECT *` is treated as potentially targeting all PII columns; behavior controlled by `wildcard_policy` setting
- Gate 1 produces a `RedactPlan` (set of forced-redaction column names + warnings) consumed by Gate 2 in the same process. Forced columns are redacted unconditionally, bypassing confidence scoring. Column aliases are resolved to their output names; schema/table qualifiers are stripped before matching

### FR-4: Gate 2 — Post-result PII scanning

- Receives JSON output (stdout) from the underlying tool
- Scans every string value in the JSON for PII patterns
- Additionally flags columns whose names match the PII column denylist regardless of value content
- Replaces detected PII values with `[PII:<type>]` (e.g. `[PII:email]`, `[PII:ssn]`)
- When multiple patterns match the same value, uses the highest-confidence match
- Non-string values (integers, booleans, nulls) are passed through unchanged
- Nested JSON (e.g. JSONB columns) is recursively scanned

### FR-5: PII pattern library

Built-in patterns (all configurable/overridable):

| Type | Detection Method |
|---|---|
| `email` | Regex |
| `ssn` | Regex (NNN-NN-NNNN format) |
| `phone` | Regex (US formats + E.164) |
| `credit_card` | Regex + Luhn algorithm check |
| `ip_address` | Regex (IPv4) |
| `date_of_birth` | Column name heuristic only (`dob`, `birthdate`, `birth_date`, `date_of_birth`) |
| `name` | Column name heuristic only (`full_name`, `first_name`, `last_name`) |

Custom patterns can be added in config as named regex strings.

### FR-6: Column name heuristics

- Maintain a built-in denylist of column name patterns that indicate PII regardless of value content
- Config can extend or override this list
- Column name matching is case-insensitive and supports substring matching (e.g. `email` matches `user_email`, `email_address`)

### FR-7: Output format

- All output is compact JSON to stdout
- Errors use `{"error": "..."}` format with exit code 1 (matching toolkit convention)
- Redacted output preserves the original structure: same keys, same row count, same field ordering
- `_redact_summary` attachment depends on the input shape (controlled by `include_summary` config flag):

  | Input shape | Detected by | Behavior |
  |---|---|---|
  | Object with `error` key | `{"error": "..."}` | Pass through unchanged. Never attach summary. |
  | Object (any other) | `{...}` | Walk for redaction. If summary enabled, set `payload["_redact_summary"] = {...}` as a sibling key. |
  | Array | `[...]` | Walk for redaction. If summary enabled, wrap as `{"rows": <original array>, "_redact_summary": {...}}`. If disabled, return the bare array unchanged. |
  | Other (`null`, scalar) | — | Pass through unchanged. |

- The array-wrapping behavior is the only case where the top-level shape changes; users opting in to `include_summary: true` for array-emitting tools (`mysql --json`, `sqlite3 -json`, etc.) accept this trade-off. Users who want strict shape preservation should set `include_summary: false`.

### FR-8: Agent harness detection

- Detect known AI agent environment variables (same list as toolkit: `CLAUDECODE`, `OPENCODE`, `COPILOT_CLI`, `COPILOT_RUN_APP`)
- Block `redact init` and interactive `redact config` (no flags) when running inside an agent harness
- Allow `redact run`, `redact hook`, `redact list`, `redact validate`, `redact config --path`, and `redact config --print` in all contexts

### FR-9: Confidence threshold

- Each regex match carries a confidence score (0.0–1.0)
- Score is computed as: pattern's base confidence, plus a column-name boost when the JSON key matches the PII denylist (capped at 1.0)
- Values matched below `confidence_threshold` (default `0.8`) are flagged in `_redact_summary` but not redacted
- Luhn-validated credit card numbers always treated as high confidence (1.0)
- Default base confidences and boosts:

  | Pattern | Base | With matching column name |
  |---|---|---|
  | `credit_card` (Luhn) | 1.00 | 1.00 |
  | `email` | 0.95 | 1.00 |
  | `ssn` | 0.90 | 1.00 |
  | `phone` | 0.70 | 0.90 |
  | `ip_address` | 0.60 | 0.85 |
  | custom | 0.80 (configurable per pattern) | +0.15 |

---

## Non-Functional Requirements

### NFR-1: Performance

- Gate 2 scanning must add less than 100ms latency for result sets up to 1000 rows × 50 columns
- Regex patterns are compiled once at startup and reused

### NFR-2: Security

- `redact` never writes query results to disk
- Config file is plaintext: it contains no credentials and therefore needs no encryption
- No PII values appear in error messages or logs
- Underlying tool credentials are not re-exposed; `redact` passes the parent env through to the subprocess unchanged and does not set, decrypt, or manage env vars itself
- Hook does not see, log, or persist credentials present in the AI's command line beyond the duration of subprocess execution

### NFR-3: Correctness

- A false negative (PII leaks through) is worse than a false positive (non-PII gets redacted)
- Default configuration should err toward redacting ambiguous matches
- Regex patterns must be tested against a corpus of real-format examples

### NFR-4: Transparency

---

## Out of Scope (v1)

- ML-based PII classification
- Streaming / chunked result scanning
- Non-JSON output formats (CSV, Parquet)
- Database-native column masking integration
- Audit logging of queries and redaction events
- Multi-language support (names, addresses in non-Latin scripts)
- IPv6 address detection
