# gate

PII-filtering CLI that transparently intercepts AI agent query commands and redacts sensitive data before it reaches the model context.

## Notes
`crates/gate/src/run.rs` is the production Gate 1 + Gate 2 pipeline. Loads config,
runs gate1::extract_columns + gate1::build_plan on the SQL arg, spawns the subprocess,
pipes stdout through common::redactor::redact. All subcommands fully implemented.

`crates/mcp/` is a separate crate that provides the `gate mcp` subcommand — a stdio
JSON-RPC proxy that forwards traffic between the AI harness and an upstream MCP server.
It intercepts `tools/call` responses and redacts PII via Gate 2 before returning them.

## Repository structure

```
gate/
  Cargo.toml            # workspace root
  crates/
    common/             # config, PII patterns, redactor (Gate 2), error types, harness detection
    gate1/              # SQL tokenizer + column extractor (Gate 1)
    gate/               # main binary (all subcommands)
    mcp/                # stdio MCP proxy: intercepts tools/call responses and redacts PII
```

## Build and test commands

```bash
cargo build
cargo test --all
cargo clippy -- -D warnings
cargo fmt --check
```

## Before every commit

Run all checks from the workspace root and fix any failures:

```bash
cargo fmt --all
cargo clippy -- -D warnings
cargo test --all
```

Never commit if any of these fail.

## Dependencies (pin these in workspace Cargo.toml)

```toml
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
serde_yaml = "0.9"
regex = "1"
shell-words = "1"
anyhow = "1"
thiserror = "1"
tempfile = "3"
criterion = { version = "0.5", features = ["html_reports"] }
ctrlc = { version = "3", features = ["termination"] }
```

Gate 1 uses a hand-written SQL tokenizer — do NOT add `sqlparser-rs`.

## Safety checklist (required for any redaction changes)

When modifying redaction logic (`common/redactor.rs`, `gate1/lib.rs`, `mcp/intercept.rs`), run this checklist before committing:

1. **False-negative scan** — identify any PII patterns that could slip through the changed logic (e.g. value types not covered by regex, Luhn bypass, forced-column path skipped).
2. **Test coverage** — for each identified gap, add a test that fails before the fix and passes after.
3. **Non-negotiables audit** — verify every item in the Non-negotiables section below is still upheld.

## Non-negotiables

- **Gate 2 false negatives are worse than false positives.** When in doubt, redact. Default config errs toward redacting ambiguous matches.
- **Never write query results to disk.** All processing is in-memory.
- **No PII in logs or error messages.** `gate hook` and `gate run` must not log the AI's command line.
- **`init.rs` writes must be atomic.** Write to a tempfile, then rename. Never write directly to `~/.claude/settings.json`.
- **`gate init` and interactive `gate config` are blocked inside agent harnesses.** Check `is_agent_harness()` at the top of those handlers.
- **`gate hook` must be fast on the passthrough path** — single-digit ms. It fires on every Bash command.
- **Errors use `{"error": "..."}` format with exit code 1**, matching toolkit convention.
- **Hook output format must match the detected input format.** Today only the snake_case Claude Code shape is implemented (`hookSpecificOutput.updatedInput`). When opencode lands, the snake_case shape is reused — the opencode plugin formats its payload as snake_case before piping to `gate hook`, so the Rust side stays single-format. Copilot CLI support is deferred.

## Key invariants by file

| File | Why it matters |
|---|---|
| `common/redactor.rs` | The load-bearing safety net. Bugs here = PII leaks. Cover with golden-file tests before trusting it. |
| `gate1/lib.rs` | Best-effort SQL parsing. Wrong here = false-negative on Gate 1, but Gate 2 catches it. Document limitations at the top of the file. |
| `gate/hook.rs` | Runs on every Bash command — both perf and correctness matter. |
| `gate/init.rs` | Touches the user's harness settings JSON. Idempotency and atomic writes are mandatory. |
| `gate/init_opencode.rs` | Writes the opencode JS plugin. Same atomicity and idempotency rules as `init.rs`. |
| `gate/run.rs` | Spawns subprocesses, handles their stdio. Most cross-component bugs live here. |
| `gate/command.rs` | Matches tool invocations in hook payloads (direct and nested via `sh -c`). Wrong here = silent passthrough of interceptable commands. |
| `gate/scan.rs` | Reads columnar schema JSON from stdin and classifies columns by PII tier. No false negatives — missing a column here gives the AI unredacted data. |
| `gate/uninstall.rs` | Removes hook entries, config dir, and opencode plugins. Must be idempotent and never delete non-gate files. |
| `gate/validate.rs` | Loads config and compiles all patterns. Must catch bad regex before the hook fires. |
| `gate/enable_disable.rs` | Edits `enabled:` in the YAML config atomically. |
| `mcp/lib.rs` | MCP proxy entry point — routes JSON-RPC between harness and upstream server. Perf matters; runs for the lifetime of the MCP server. |
| `mcp/intercept.rs` | Decides which MCP messages carry tool results and redacts them. Bugs here = PII leaks through the MCP path. |

## Testing approach

- Write tests **before** or **alongside** code changes, not after.
- `common/redactor.rs` requires golden-file tests with realistic PII data. False-negative rate on the test corpus must be 0.
- Integration tests for `gate run` use a fake-tool binary (a shell script emitting known JSON for known SQL).
