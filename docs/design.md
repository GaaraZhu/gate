# Design: redact

## Overview

`redact` is a standalone Rust CLI that acts as a transparent PII-filtering proxy between AI agents and query tools. Installation registers a PreToolUse hook in the agent harness; when the AI tries to run a configured query tool, the hook silently routes the command through `redact run`, which applies two sequential detection gates and returns sanitized JSON. The AI agent sees the same JSON structure as before, with PII values replaced by typed placeholders. Humans and CI scripts running outside the agent harness are unaffected — no wrapper scripts are installed on PATH.

---

## Relationship to toolkit

`redact` is a separate binary and repository from `toolkit`. The two are independently optional and composable:

- `toolkit` enforces **inbound** controls: write protection, credential hiding, command allow/deny rules
- `redact` enforces **outbound** controls: PII detection and redaction on query results

`redact` intercepts `tkpsql` / `tkdbr` / raw clients via a PreToolUse hook. Neither tool depends on the other at the code level.

```
toolkit layer:  tkpsql              →  PostgreSQL    (write guard, credential injection)
redact layer:   PreToolUse hook     →  tkpsql        (PII scan on returned JSON)
```

### Defense-in-depth model

`redact` is one layer in a three-layer stack. Users compose layers based on their threat model:

| Layer | Threat | Notes |
|---|---|---|
| Agent harness sandbox | AI invokes raw clients directly, bypassing the hook chain | Outside `redact`'s scope; provided by Claude Code, etc. |
| toolkit | Write ops, credential exposure | Optional but recommended for production |
| redact | PII in query results | This component |

`redact` makes **no claims** about credential protection. When wrapping toolkit, credentials are protected by toolkit. When wrapping a raw client (`mysql`, `psql`), credentials reachable to the wrapped tool (e.g. `~/.my.cnf`, `~/.pgpass`) are also reachable to the AI — `redact` does not change that. See the Credentials subsection below.

---

## Interception Model

`redact` does **not** generate wrapper scripts on PATH. Instead, it registers a PreToolUse hook in the agent harness. Every Bash command the AI tries to run is offered to `redact hook` first; commands that match a configured tool are routed through `redact run`. Humans and CI scripts running outside the agent harness see no change in behavior — the hook only fires inside the harness.

This closes the bypass surface that wrapper scripts left open: the AI cannot invoke `tkpsql` directly to skip filtering, because every Bash invocation passes through the hook.

### Per-harness installation surface

| Harness | Hook config location | Written by `redact init --harness …` | Status |
|---|---|---|---|
| Claude Code | `~/.claude/settings.json` (`hooks.PreToolUse[]`, PascalCase) | `claude-code` (default) | Shipped |
| opencode | TypeScript plugin file at `~/.config/opencode/plugin/redact.ts` (global, default) or `./.opencode/plugin/redact.ts` (with `--scope project`). The plugin's `tool.execute.before` hook mutates `output.args.command` for the bash tool. | `opencode` | Planned (plan.md Milestone 9) |
| GitHub Copilot CLI | `./.github/hooks/hooks.json` (project-scoped, `hooks.preToolUse[]`, lowerCamelCase per [GitHub's hooks reference](https://docs.github.com/en/copilot/reference/hooks-configuration)) **plus** `.github/copilot-instructions.md` snippet | `copilot-cli` | **Deferred** (advisory enforcement only — see Enforcement Strength below; spec retained in plan.md Milestone 8) |

VS Code Copilot Chat is supported by the same `redact hook` binary (its hook input shape matches Claude Code's snake_case format), but its installation is part of the user's VS Code configuration and is **not** managed by `redact init` in v1.

### Hook input/output format (single shape)

`redact hook` accepts one input shape — the snake_case PreToolUse JSON used by Claude Code and VS Code Copilot Chat — and emits one response shape:

| Detected shape | Source | Response on intercept |
|---|---|---|
| `{"tool_name":"Bash","tool_input":{"command":"…"}}` | Claude Code, VS Code Copilot Chat, **opencode** (via the bundled plugin's translation layer) | `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{"command":"redact run -- …"}}}` — transparent rewrite |
| Anything else | — | Passthrough (no stdout output, exit 0) |

Per-harness translation lives **outside** `redact hook`:

- **Claude Code / VS Code Copilot Chat** speak this shape natively.
- **opencode** uses a TypeScript plugin (written by `redact init --harness opencode`) that translates opencode's `tool.execute.before(input, output)` call into the snake_case JSON above, pipes it to `redact hook`, parses `hookSpecificOutput.updatedInput.command` from the response, and assigns it to `output.args.command`. The mutation propagates to the bash subprocess opencode spawns — same enforcing guarantee as Claude Code.
- **GitHub Copilot CLI** would need a different output shape (`permissionDecision: "deny"` + reason — Copilot CLI does not accept `updatedInput`). That path is specced in plan.md Milestone 8 but deferred; the deferral keeps `redact hook` to a single output contract and avoids shipping the advisory-only enforcement model. See requirements.md "Enforcement strength varies by harness" for the full trade-off.

---

## Call Chain

```
AI Agent (inside Claude Code)
  │
  │  asks to run: tkdbr --connection dev query --sql "SELECT id, email FROM users"
  ▼
Claude Code Bash tool → PreToolUse hook
  │
  ▼
redact hook        (reads command from stdin)
  │
  │  argv[0] == "tkdbr" → matched in tools: config
  │  rewrites to: redact run -- tkdbr --connection dev query --sql "..."
  ▼
redact run
  │
  ├─► [Gate 1: Pre-query SQL inspection]
  │     - Look up tools["tkdbr"].sql_arg → "--sql"
  │     - Extract SQL from the forwarded command's --sql arg
  │     - Parse SQL → identify referenced column names
  │     - Match against PII column denylist
  │     - Build RedactPlan (forced columns + warnings)
  │     - SELECT * triggers wildcard_policy
  │
  │  spawns subprocess: tkdbr --connection dev query --sql "..."
  ▼
tkdbr → Databricks
  │
  ◄── raw JSON rows ──────────────────────────────────────────────
  │   {"rows": [{"id": 1, "email": "alice@example.com"}], "count": 1}
  │
  ▼
redact run receives captured stdout
  │
  ├─► [Gate 2: Post-result PII scanning]
  │     - Parse JSON; for each string value:
  │         a. Forced column? → redact unconditionally
  │         b. Column-name heuristic + regex + Luhn
  │     - Confidence scoring → redact if ≥ threshold
  │     - Replace value with [PII:<type>]
  │     - Optionally append _redact_summary
  │
  ▼
Sanitized JSON returned to AI
  {"rows": [{"id": 1, "email": "[PII:email]"}], "count": 1, "_redact_summary": {...}}
```

---

## Two-Gate Model

### Gate 1 — Pre-query (SQL inspection)

**Purpose:** Block or plan redaction before data moves. Cheap, deterministic, catches the obvious.

**How it works:**
1. Scan command arguments for `--sql <stmt>` or `--sql=<stmt>`
2. Extract column references using lightweight SQL token scanning (not a full parser):
   - Pull identifiers from `SELECT <cols> FROM`
   - Strip aliases, function wrappers, schema qualifiers
   - `SELECT *` → treat as unknown column set, apply `wildcard_policy`
3. Match column names against denylist (case-insensitive, substring match)
4. Apply configured `action`:
   - `warn`: log warning, proceed
   - `redact`: mark columns for guaranteed redaction in Gate 2 regardless of value content
   - `reject`: return error JSON, exit 1

**Limitations:**
- Cannot handle deeply nested subqueries perfectly (best-effort)
- Cannot resolve `*` to actual column list without schema lookup (v1: treated as unknown)
- Does not parse non-SQL query formats (Databricks SQL is similar enough to work)

### Gate 2 — Post-result (value scanning)

**Purpose:** Safety net. Catches anything Gate 1 missed: dynamically-named columns, free-text fields containing PII, columns not referenced explicitly in SQL.

**How it works:**
1. Parse stdout as JSON
2. Walk every value in the JSON tree recursively
3. For each string value, run detection pipeline:
   - Column name heuristic (using the key name at current JSON path)
   - Regex match against compiled pattern library
   - Luhn check for credit-card-shaped strings
4. Compute confidence score per match: `base_confidence + (column_name_boost if key matches denylist else 0)`, capped at 1.0
5. When multiple patterns match the same value, keep the highest-scoring match
6. Redact values at or above `confidence_threshold`
7. Values below threshold: include in `_redact_summary` as warnings only
8. Nested JSON strings (JSONB columns) are parsed and recursively scanned

### Gate 1 → Gate 2 Handoff

Both gates run in the same process: `redact run` parses args, executes Gate 1 against the SQL, spawns the underlying tool as a subprocess, captures stdout, and feeds it to Gate 2. The "handoff" is a plain in-process struct.

**Shared object:**

```rust
pub struct RedactPlan {
    /// Columns Gate 1 marked for guaranteed redaction.
    /// Key = JSON key name (lowercased, alias-resolved).
    /// Value = the PII type label to use in the [PII:<type>] placeholder.
    pub forced_columns: HashMap<String, String>,

    /// Warnings to merge into _redact_summary regardless of Gate 2's findings.
    /// E.g. "SELECT * encountered, wildcard_policy=warn"
    pub warnings: Vec<String>,

    /// True if Gate 1 already terminated the run (action=reject).
    /// In that case redact has exited before Gate 2 runs.
    pub rejected: bool,
}
```

**Gate 1 populates it per configured `action`:**

| Finding | `action` | Effect |
|---|---|---|
| Denylisted column in SELECT | `redact` | Insert into `forced_columns` with the matched denylist label |
| Denylisted column in SELECT | `warn` | Append to `warnings`; do not mark for forced redaction |
| Denylisted column in SELECT | `reject` | Exit 1 with error JSON; Gate 2 never runs |
| `SELECT *` | `wildcard_policy: warn` | Append a wildcard warning |
| `SELECT *` | `wildcard_policy: reject` | Exit 1 |
| No `--sql` arg | n/a | Empty plan; Gate 2 runs normally |

**Gate 2 consumes it as the first step of its per-value pipeline:**

```
for (key, value) in walk_json(payload):
    if key.to_lowercase() in plan.forced_columns:
        replace value with "[PII:{plan.forced_columns[key]}]"
        record in summary; skip remaining detection
    else:
        run normal pipeline (column-name heuristic, regex, Luhn,
                             confidence scoring vs threshold)

merge plan.warnings into _redact_summary.warnings
```

The forced-column path bypasses confidence scoring entirely — `action: redact` is a hard decision.

**Edge cases:**
- **Column aliases** (`SELECT email AS contact`): Gate 1 stores the alias (`contact`) as the key, since that's what appears in the JSON. The original column name (`email`) becomes the type label, so the placeholder is `[PII:email]`.
- **Qualified columns** (`u.email`, `users.email`): Gate 1 strips the qualifier before denylist matching and before storing the key.
- **No SQL present**: empty plan; Gate 2 runs normally.
- **Tool returned an error JSON** (`{"error": "..."}`): Gate 2 detects the error shape and passes it through unchanged, ignoring the plan.
- **Forced column also matches a regex**: forced path wins; no double-processing.

---

## Credentials

`redact`'s config contains no credentials. Authentication to the underlying datastore is the wrapped tool's responsibility.

**Toolkit-wrapped connections (recommended for production):**
Credentials live in toolkit's encrypted config. Toolkit decrypts on demand and injects credentials into the subprocess env at spawn time. The AI sees neither the ciphertext path nor the plaintext credential.

**Raw-client connections (escape hatch):**
The wrapped tool reads its credentials from its standard locations (`~/.my.cnf`, `~/.pgpass`, env vars, etc.). Those locations are also readable by the AI agent — `redact` cannot prevent that. This configuration is acceptable when the credential is intentionally low-sensitivity (read-only DB user, local dev DB), but should not be the default for production deployments.

**Enforced rules:**
- The `tools:` config has no fields that hold credentials. There is no `command:` field for users to fill — the spawn target is whatever the AI typed.
- `redact` passes the parent process's env through to the subprocess unchanged but does not set, decrypt, or manage env vars itself.
- `redact validate` emits a soft warning when a configured tool entry is a raw client (`mysql`, `psql`, `mongosh`, `mysqlsh`, `sqlite3`) rather than a toolkit wrapper, surfacing the credential-exposure trade-off without blocking the configuration.
- `redact hook` and `redact run` do not log the AI's command line at any level (info/debug); credentials in argv must not survive in any logs.

---

## Configuration

```yaml
# ~/.config/redact/config.yaml

# Tools to intercept. Key = executable basename (matched against argv[0]).
# `sql_arg` is the flag the tool uses to receive SQL.
# Set sql_arg: null to disable Gate 1 for that tool (output-only filtering).
tools:
  tkpsql:
    sql_arg: "--sql"
  tkdbr:
    sql_arg: "--sql"
  mysql:
    sql_arg: "-e"
  psql:
    sql_arg: "-c"
  # sqlite3 takes SQL as a positional arg, not behind a flag:
  # sqlite3:
  #   sql_arg: null

pii:
  # Built-in column name denylist (extended by this list)
  column_names:
    - email
    - ssn
    - dob
    - phone
    - npi          # National Provider Identifier
    - credit_card
    - card_number
    - cvv
    - passport
    - license_number

  # Gate 1 behavior when a denylisted column is found in SELECT
  action: redact         # warn | redact | reject

  # Gate 1 behavior when SELECT * is used
  wildcard_policy: warn  # warn | reject

  # Gate 2 regex patterns (built-ins, overridable)
  # Each pattern: regex + base confidence (0.0-1.0)
  patterns:
    email:
      regex: '[\w.+\-]+@[\w\-]+\.[a-z]{2,}'
      confidence: 0.95
    ssn:
      regex: '\b\d{3}-\d{2}-\d{4}\b'
      confidence: 0.90
    phone:
      regex: '\b(\+1[\s.]?)?\(?\d{3}\)?[\s.\-]\d{3}[\s.\-]\d{4}\b'
      confidence: 0.70
    ip:
      regex: '\b(?:\d{1,3}\.){3}\d{1,3}\b'
      confidence: 0.60
    # credit_card: handled by Luhn algorithm; confidence fixed at 1.0

  # Bonus added to a pattern's base confidence when the JSON key
  # for the matched value also appears in the column-name denylist.
  # Final score is capped at 1.0.
  column_name_boost: 0.15

  confidence_threshold: 0.8

  # Redaction placeholder template; {type} is replaced with pattern name
  redaction: "[PII:{type}]"

  # Append _redact_summary to response JSON
  include_summary: true
```

---

## CLI Surface

`redact` uses explicit verb subcommands. The hook (`redact hook`) and worker (`redact run`) are invoked by the agent harness, not by users. Setup commands (`redact init`, `redact config`) are interactive and blocked inside agent harnesses.

| Command | Purpose | Blocked in agent harness (FR-8)? |
|---|---|---|
| `redact run -- <tool args...>` | Execute the wrapped tool with Gate 1 + Gate 2. Looks up `tools[argv[0]].sql_arg` from config to find the SQL flag. | No |
| `redact hook` | PreToolUse callback: read Bash command from stdin, decide intercept vs. passthrough, emit (possibly rewritten) command to stdout | No |
| `redact init [--harness claude-code\|opencode] [--scope project\|global]` | Register the PreToolUse hook (Claude: `~/.claude/settings.json`; opencode: TypeScript plugin file at the chosen scope). `copilot-cli` deferred — see plan.md Milestone 8. | Yes |
| `redact config [--path \| --print \| --init-only]` | Manage `redact`'s own config file (interactive edit by default) | Yes (interactive); `--path`/`--print` allowed |
| `redact list` | Print configured tools and their `sql_arg` values | No (read-only) |
| `redact validate` | Load config, compile patterns, report errors and soft warnings | No |
| `redact version` | Print version | No |

**Hook entries written by `redact init`:**

*Claude Code* — `~/.claude/settings.json`:

```jsonc
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "redact hook" }] }
    ]
  }
}
```

*opencode* (planned, plan.md Milestone 9) — TypeScript plugin file at `~/.config/opencode/plugin/redact.ts` (default global; `--scope project` writes `./.opencode/plugin/redact.ts` instead). The plugin's `tool.execute.before` hook synchronously calls `redact hook` with snake_case JSON, parses the response, and mutates `output.args.command`. Sketch:

```ts
import { spawnSync } from "node:child_process"

export const RedactPlugin = async () => ({
  "tool.execute.before": async (input, output) => {
    if (input.tool !== "bash") return
    const cmd = output.args.command
    if (typeof cmd !== "string" || cmd.length === 0) return
    const result = spawnSync("redact", ["hook"], {
      input: JSON.stringify({ tool_name: "Bash", tool_input: { command: cmd } }),
      encoding: "utf8",
      timeout: 5000,
    })
    if (result.status !== 0 || !result.stdout) return
    try {
      const updated = JSON.parse(result.stdout)?.hookSpecificOutput?.updatedInput?.command
      if (typeof updated === "string" && updated.length > 0) {
        output.args.command = updated
      }
    } catch { /* malformed → passthrough */ }
  },
})
```

The mutation propagates because opencode's plugin contract treats `output.args` as mutable (verified in `sst/opencode:packages/plugin/src/index.ts`).

*GitHub Copilot CLI* — **deferred** to a future release. See plan.md Milestone 8 for the full schema and `init_copilot` design (`./.github/hooks/hooks.json` with `version: 1` + lowerCamelCase `preToolUse[]`, plus `.github/copilot-instructions.md` anchor block).

**Hook decision logic (`redact hook`):**

```
read full JSON request from stdin
parse as snake_case PreToolUse shape; if not parseable → passthrough (no output, exit 0)
extract command from tool_input.command

parse tokens; find the configured tool token (basename)
if tool basename not in tools:               # passthrough
    write nothing, exit 0
if command already starts with "redact run": # loop avoidance
    write nothing, exit 0

build new_command = "redact run -- <original command>"
emit hookSpecificOutput{ permissionDecision: allow,
                         updatedInput: { command: new_command } }
exit 0
```

opencode reuses this same pipeline — its plugin is responsible for converting opencode's `tool.execute.before` arguments into the snake_case shape on the way in, and for assigning `hookSpecificOutput.updatedInput.command` back to `output.args.command` on the way out. Copilot CLI would need a different output shape; that work is deferred (plan.md Milestone 8).

Agent-harness gating is a single `is_agent_harness()` check at the top of the `init` and interactive-`config` handlers. The hook itself runs unconditionally — it must, since its whole purpose is to fire inside the agent harness.

---

## Repository Structure

```
redact/
  Cargo.toml            # workspace
  crates/
    common/             # config loading, PII pattern library, error types
      src/
        config.rs       # load_config(), tools lookup
        patterns.rs     # compiled regex library, Luhn check, confidence scoring
        redactor.rs     # Gate 2: walk JSON tree, apply patterns, return redacted Value
        error.rs        # ErrorResponse, exit_with_error()
    gate1/              # SQL token scanner for pre-query column extraction
      src/
        lib.rs          # extract_columns(sql: &str) -> Vec<String>
        tokenizer.rs    # lightweight SQL tokenizer (no full parse tree needed)
    redact/             # main binary
      src/
        main.rs         # CLI entrypoint, clap subcommand dispatch
        run.rs          # `redact run`: spawn subprocess, capture stdout, apply Gate 1 + Gate 2
        hook.rs         # `redact hook`: PreToolUse callback (format detection, decide rewrite, dual output)
        init.rs         # `redact init`: dispatches per --harness; Claude Code path
        init_opencode.rs # `redact init --harness opencode`: writes ~/.config/opencode/plugin/redact.ts (planned, plan.md M9)
        # init_copilot.rs # `redact init --harness copilot-cli`: deferred — see plan.md M8
        config_cmd.rs   # `redact config`: create starter config + open in $EDITOR
        list.rs         # `redact list`: print configured tools
        validate.rs     # `redact validate`: parse config + compile patterns + soft warnings
        harness.rs      # is_agent_harness() detection (FR-8)
        starter.rs      # embedded starter-config template
```

---

## Key Dependencies (Rust crates)

| Crate | Purpose |
|---|---|
| `clap` | CLI argument parsing |
| `serde` + `serde_json` | JSON parsing and traversal |
| `serde_yaml` | Config file parsing |
| `regex` | Compiled PII pattern matching |
| `shell-words` | Parse the AI's incoming Bash command line in `redact hook` |

SQL parsing for Gate 1 uses a hand-written tokenizer rather than `sqlparser-rs`. Rationale: we only need to extract column names from `SELECT <cols> FROM`, not build a full AST. A tokenizer is ~100 lines, has no transitive dependencies, and handles the Databricks SQL dialect without configuration. `sqlparser-rs` would add significant compile time and complexity for a task that is fundamentally best-effort anyway.

---

## Output Format

Different tools emit different top-level JSON shapes. `redact` adapts based on what it sees, with the goal of preserving the original shape whenever possible.

**Shape detection and handling:**

```
detect_shape(payload):
  if payload is an object with key "error" → ERROR
  if payload is an object → OBJECT
  if payload is an array  → ARRAY
  else → OTHER (null, scalar)

handle(payload, summary):
  ERROR  → return payload unchanged; never attach summary
  OBJECT → walk for redaction; if include_summary, set payload["_redact_summary"] = summary
  ARRAY  → walk for redaction; if include_summary, return {"rows": <array>, "_redact_summary": summary}
                              ; else return the bare array unchanged
  OTHER  → return payload unchanged
```

Gate 2's tree walk is the same regardless of shape — it just walks JSON and redacts string values. The shape logic only governs where (or whether) `_redact_summary` attaches.

**Object input (e.g. `tkpsql`):**
```json
// Input
{"rows": [{"id": 1, "email": "alice@example.com", "ssn": "123-45-6789"}], "count": 1}

// Output
{
  "rows": [{"id": 1, "email": "[PII:email]", "ssn": "[PII:ssn]"}],
  "count": 1,
  "_redact_summary": {"redacted": 2, "types": ["email", "ssn"], "warnings": []}
}
```
Shape preserved exactly; summary added as a sibling key.

**Array input (e.g. `mysql --json`) with `include_summary: true`:**
```json
// Input
[{"id": 1, "email": "bob@example.com"}]

// Output (wrapped)
{
  "rows": [{"id": 1, "email": "[PII:email]"}],
  "_redact_summary": {"redacted": 1, "types": ["email"], "warnings": []}
}
```
Wrapping is the documented trade-off for array-emitting tools when summary is enabled. Users who require strict shape preservation must set `include_summary: false`.

**Array input with `include_summary: false`:**
```json
// Input
[{"id": 1, "email": "bob@example.com"}]

// Output (shape preserved)
[{"id": 1, "email": "[PII:email]"}]
```

**Error pass-through (matching toolkit convention):**
```json
{"error": "query targets PII column: ssn"}
```
Errors are detected by the presence of an `error` key on an object payload and forwarded unchanged. `_redact_summary` is never attached to error responses.

**Other shapes (`null`, scalar):** passed through unchanged. These are unusual for a database client but cheap to handle.

---

## Design Decisions & Trade-offs

**Why not integrate into toolkit?**
`redact` has a different trust boundary and deployment lifecycle. Teams may want PII filtering without adopting toolkit's full credential management. Keeping them separate allows independent versioning and adoption.

**Why subprocess model instead of HTTP proxy?**
AI agents interact with CLIs, not HTTP. A subprocess model requires no infrastructure, no ports, no TLS config. It composes naturally with existing shell-based workflows.

**Why a PreToolUse hook instead of generated wrapper scripts on PATH?**
Wrapper scripts coexist with the underlying tool on PATH, so the AI can bypass redact by calling `tkpsql` directly. They also pollute PATH, require a per-connection `redact install` step, and require teaching the AI (via CLAUDE.md) to use the wrapper names. A PreToolUse hook closes the bypass surface (every Bash command is intercepted), needs no wrappers on disk, and requires no AI awareness of `redact`. The trade-off is portability: the hook only works in agent harnesses that support pre-execution hooks (v1 = Claude Code). Users running outside a supported harness see no filtering, which is the correct behavior — non-AI consumers don't want their tool output mangled.

**Why drop per-connection config in favor of per-tool config?**
The connection identity (`--connection prod`) is already in the AI's command line; `redact` doesn't need to model it. The only thing that varies between tools is the SQL flag (`--sql` vs `-e` vs `-c`), and that varies by tool, not by connection. Keying config on tool keeps the schema minimal and avoids duplicating information that already exists in argv.

**Why redact instead of block?**
Blocking breaks the AI's ability to reason about data shape ("I can't see any data"). Typed placeholders (`[PII:email]`) let the AI know the field exists and what kind of data it holds, enabling useful reasoning like "there are 500 users, but their emails are redacted" without exposing values.

**Why hand-written SQL tokenizer instead of sqlparser-rs?**
Gate 1 is best-effort by design — we only need column names from the SELECT list. A tokenizer handles this in ~100 lines with no dependencies. Full AST parsing adds compile weight and dialect configuration complexity for marginal gain, since Gate 2 catches anything Gate 1 misses.

**Why confidence threshold instead of binary match?**
Some patterns (short phone numbers, 9-digit IDs that look like SSNs) have high false-positive rates. A threshold allows tuning without changing patterns, and surfaces low-confidence matches as warnings so operators can decide.

---

## Open Questions (to resolve during implementation)

1. **SELECT \* handling:** v1 will skip schema lookup and rely on Gate 2 (resolved).
2. **Encrypted config:** Resolved as **no** — `redact`'s config contains no credentials, so plaintext is fine. Encryption belongs in toolkit's layer.
3. **Per-tool PII overrides:** Should the `tools:` entries be able to override the global PII pattern list (e.g. healthcare NPI for a specific tool)? Likely yes in a later version; v1 keeps the policy global to keep the schema small.
4. **Tool returns an error:** Gate 2 detects the `{"error": "..."}` shape and passes it through unchanged (resolved).
5. **JSON output shape:** Resolved. Adaptive handling per the shape-detection table in the Output Format section: object inputs get `_redact_summary` attached as a sibling; array inputs are wrapped as `{"rows": ..., "_redact_summary": ...}` only when `include_summary: true`; errors pass through unchanged.
6. **Pattern-matching robustness in `redact hook`:** Basename-only matching of `argv[0]` is the v1 plan. Open: how to handle aliases (e.g. user shell aliases `tk=tkpsql`), shell pipelines (`tkpsql ... | jq ...`), and `env VAR=val tkpsql ...`. v1 likely starts with simple basename matching and adds smarter parsing if false negatives bite.
7. **Multi-harness support:** v1 shipped Claude Code. Milestone 9 adds opencode (transparent rewrite via `tool.execute.before` plugin — enforcing). VS Code Copilot Chat is compatible by reusing the snake_case shape but its install lives in the user's VS Code config (no `redact init` mode in v1). GitHub Copilot CLI (deny-with-suggestion, advisory) is specced in Milestone 8 but deferred. Cursor, Gemini CLI, and Aider remain incremental work behind `redact init --harness <name>`.
8. **Audit logging:** Out of scope for v1, but the summary field in output is the foundation. A future `--audit-log` flag could append redaction events to a file.
