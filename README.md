<p align="center">
  <img src="assets/banner.png" alt="gate" width="600">
</p>

<p align="center">
  <strong>A deterministic privacy boundary between your data and AI.<br>Intercepts query results before the model sees them — rule-driven, reproducible, and audit-ready.</strong>
</p>

<p align="center">
  <a href="https://github.com/GaaraZhu/gate/actions"><img src="https://github.com/GaaraZhu/gate/workflows/CI/badge.svg" alt="CI"></a>
  <a href="https://github.com/GaaraZhu/gate/releases"><img src="https://img.shields.io/github/v/release/GaaraZhu/gate" alt="Release"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="https://github.com/GaaraZhu/homebrew-gate"><img src="https://img.shields.io/badge/homebrew-tap-orange?logo=homebrew" alt="Homebrew"></a>
</p>

<p align="center">
  <strong>English</strong> | <a href="README.zh-CN.md">简体中文</a>
</p>

---

AI agents increasingly access internal databases and APIs through CLI tools, scripts, and MCP servers. Without safeguards, sensitive data such as emails, phone numbers, tax identifiers, and payment details can be unintentionally exposed to LLM context windows.

`gate` intercepts query results before they reach the model and automatically redacts detected PII fields without requiring changes to existing agent workflows or prompts. It covers both access paths agents use: **Bash commands** (via a harness hook) and **MCP server calls** (via a wrap-style stdio proxy), adding < 10 ms of overhead per query.

## Why rules, not a model?

Most PII guardrails for AI agents are themselves LLMs — they send your data to a model to decide whether it's sensitive. Gate takes the opposite approach.

| | gate | LLM-based redaction |
|---|---|---|
| Decision method | Regex + column heuristics + Luhn | Model inference |
| Deterministic | ✅ Same input always produces the same output | ❌ Varies by run and model version |
| Data stays local | ✅ Never leaves your machine | ❌ Sent to a model API for classification |
| Latency | ✅ < 10ms overhead | ❌ Adds an API round-trip |
| Auditable | ✅ Every decision traceable to an explicit rule | ❌ Model reasoning is opaque |
| Known gaps | ✅ Documented — free-text prose | ❌ False-negative rate unknown |
| Redaction guarantee | ✅ 100% for configured tools — automated detection + `pii.column_denylist` closes any remaining gaps | ❌ No deterministic guarantee |

The trade-off gate makes: rules can't catch PII in unstructured free-text prose. The [threat model](THREAT-MODEL.md) documents what gate doesn't cover.

## Why not just mask at the source?

Database-level masking is the right answer when you control the source. Gate fills the gap when you don't, and covers the paths masking can't reach.

| | gate | Database masking |
|---|---|---|
| Requires DB admin access | ✅ No changes to the database | ❌ Needs column-level config by a DBA |
| Works on vendor / external DBs | ✅ Wraps any JSON-returning tool | ❌ Only databases you administer |
| Covers MCP and API tools | ✅ Any `tools/call` response | ❌ No masking concept at this layer |
| Production data freshness | ✅ Works against live data | ❌ Static copies drift; DDM may lag |
| Agent bypass resistance | ✅ Direct value exposure blocked in harness hook | ❌ Aggregate functions and CASE expressions can bypass DDM |
| Known gaps | ✅ Documented | ❌ DDM gaps are often silent |

They're complementary: if you have DDM configured, gate is the safety net for the paths and patterns DDM misses.

## Demo

The demo walks through three steps:

1. `gate scan` detecting PII columns across the schema before any query runs
2. An agent querying the transactions table with gate disabled — `card_number` fully visible
3. The same queries with gate enabled — `card_number` redacted across both MCP and Bash paths

![gate intercepting PII before it reaches the model](assets/demo.gif)

> For the design rationale, threat-model walkthrough, and detection-pipeline deep dive, read [**Introducing gate**](https://gaarazhu.github.io/introducing-gate/).

## Scan your schema

Before installing the hook, use `gate scan` to assess how much PII your schema exposes. Pipe a `TABLE_NAME, COLUMN_NAME` query into it and gate prints a risk report across every table. No config is required for `gate scan` itself.

```bash
psql -U <user> -h <host> -d <dbname> -c "SELECT TABLE_NAME, COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = 'public' ORDER BY TABLE_NAME, ORDINAL_POSITION" | gate scan
```

See [docs/scan.md](docs/scan.md) for queries against MySQL, MS SQL Server (including native `sqlcmd`), Databricks, and toolkit-managed clients — and for the full risk-scoring breakdown, denylist and allowlist guidance.

## Quickstart

1. **Install gate** (pick one)

   ```bash
   brew tap GaaraZhu/gate && brew install gate  # Homebrew — macOS and Linux (recommended)
   cargo binstall gate                           # cargo binstall — downloads a prebuilt binary
   ```

   Or grab a binary directly from the [releases page](https://github.com/GaaraZhu/gate/releases).

   On Homebrew 6.0+, third-party taps must be explicitly trusted before their formulae can be installed. If `brew install gate` fails with a trust error, run `brew trust GaaraZhu/gate` and retry.

2. **Create your config** (opens `~/.config/gate/config.yaml` in your editor):

   ```bash
   gate config
   ```

3. **Register the hook** with your agent harness:

   ```bash
   gate init                            # Claude Code (default)
   gate init --harness opencode         # OpenCode
   gate init --harness cursor           # Cursor
   gate init --harness copilot-cli      # GitHub Copilot CLI (project-scoped, run from repo root)
   gate init --harness codex            # Codex CLI
   gate init --harness gemini           # Gemini CLI
   gate init --harness codebuddy        # CodeBuddy
   ```

   Add `--scope project` for project-only setup. Restart your OpenCode, Cursor, Gemini CLI, or CodeBuddy session after `gate init` to load the hook. For Codex CLI, restart the session, then review the hook in the Trust & Permissions UI, mark it as trusted, and enable it. For Copilot CLI, add `.github/hooks/PreToolUse.json` to your repo's `.gitignore` — each developer runs `gate init --harness copilot-cli` once in their local clone.

4. *(Optional)* **Register MCP server proxies** so `tools/call` responses also pass through gate:

   ```bash
   gate init --wrap-mcp                          # Claude Code (default) — dry-run, shows what would change
   gate init --harness opencode --wrap-mcp --yes # OpenCode
   gate init --harness cursor --wrap-mcp --yes   # Cursor
   gate init --harness copilot-cli --wrap-mcp --yes # GitHub Copilot CLI
   gate init --harness codex --wrap-mcp --yes    # Codex CLI
   gate init --harness gemini --wrap-mcp --yes   # Gemini CLI
   gate init --harness codebuddy --wrap-mcp --yes # CodeBuddy
   ```

   Add `--scope project` for project-level MCP config. For Cursor project-scoped MCP, re-enable the servers in **Settings → Tools & MCPs** after registration. See [docs/mcp.md](docs/mcp.md) for `--servers`, per-harness paths, and manual single-server registration.

5. **Start your AI session** — `gate` intercepts query commands automatically. No changes to your prompts or tools required.

Run `gate validate` to confirm your config is valid before the first session.

## Team setup

Personal config (`~/.config/gate/config.yaml`) doesn't travel with the repo — one developer tightening a threshold or adding a pattern doesn't help anyone else. `.gate/config.yaml`, committed to git, fixes that.

```bash
# Team lead, once:
gate export                          # writes personal config's tools/patterns/thresholds into .gate/config.yaml
git add .gate/config.yaml && git commit -m "add gate team config"

# Everyone else (after gate init from the Quickstart above):
git clone <repo>
gate config --sync                   # scaffolds/picks up .gate/config.yaml, merges it into personal config
```

The merge is tighten-only — a project config can raise thresholds and add tools/patterns, never weaken protection — with one called-out exception for `column_allowlist`. See [docs/team.md](docs/team.md) for the full merge rules and why `gate config --sync` bakes it into personal config instead of only reading it live.

## How it works

`gate` covers two access paths agents use to reach data. The [blog post](https://gaarazhu.github.io/introducing-gate/) has the full walkthrough; the short version:

### Bash tooling path

Every Bash command passes through `gate hook` first. Commands that match a configured tool are silently rewritten to `gate run -- <original command>`, which spawns the subprocess and pipes stdout through the two-gate detection pipeline. The rewrite is enforcing — the agent cannot bypass it.

```
AI asks to run: tkpsql query --sql "SELECT * FROM users"
                        │
         harness hook fires (PreToolUse / tool.execute.before)
                        │
              gate hook rewrites to: gate run -- tkpsql query --sql "..."
                        │
         ┌──────────────┴──────────────┐
         │ Gate 1: SQL inspection      │  SELECT * → no column hints, defer to Gate 2
         │ Gate 2: Value scanning      │  regex + column-name heuristics + Luhn check
         └──────────────┬──────────────┘
                        │
         {"id": 1, "full_name": "[PII:name]", "email": "[PII:email]", ..., "_gate_summary": {...}}
```

### MCP path

`gate mcp` is a transparent stdio proxy registered in the harness as the MCP server. It forwards all JSON-RPC traffic verbatim except `tools/call` responses, which pass through Gate 2 before reaching the model. No changes to the upstream server are required.

> **Note:** only `tools/call` responses are redacted — `resources/read`, `prompts/get`, and other MCP message types are forwarded without inspection.

```
AI ──tools/call──> gate mcp ──forward──> upstream MCP server
                       │
                       │ <── tools/call response with PII
                       │
                       │ Gate 2 scan + redact
                       │
AI <───redacted result─┘
```

### Output

PII values are replaced with `[PII:<type>]` placeholders; original JSON structure is preserved. A `_gate_summary` field is appended reporting what was redacted.

```json
{
  "rows": [{"id": 1, "email": "[PII:email]", "ssn": "[PII:ssn]"}],
  "count": 1,
  "_gate_summary": {"redacted": 2, "types": ["email", "ssn"], "warnings": []}
}
```

See [docs/configuration.md](docs/configuration.md) for output options including deterministic value hashing.

## What gate does NOT protect against

`gate` is a deterministic redaction layer, not a sandbox. Key limitations:

- **Commands not in `tools:`.** The AI can invoke them freely; their output is never inspected.
- **Non-JSON tool output.** Plain text, CSV, and other formats pass through unchanged.
- **Free-text prose.** PII embedded in unstructured text fields is not detected.
- **Adversarial agents / prompt injection.** Gate assumes the agent inadvertently exfiltrates PII — a deliberate attacker can route around it.
- **PII already in the model's context** from prior turns, system prompts, or file reads.

See [THREAT-MODEL.md](THREAT-MODEL.md) for the full attacker model, known bypasses, and recommended configuration for stricter enforcement.

## Protection retrospective

`_gate_summary` reports a single response. `gate retro` aggregates across all of them — total queries seen, PII fields redacted, hit rate, plus a breakdown by tool and PII category. Useful for periodic audits and for confirming the boundary is doing real work.

If any query produced a low-confidence redaction, `gate retro` surfaces a **Low-confidence redactions** section listing each unique warned column and the exact `gate allowlist add <col>` command to suppress it. Once a column is added to the allowlist it disappears from this section automatically.

![gate retro output](assets/retro.jpg)

Stats are collected by default and written to a local JSONL log on disk — they never leave your machine. Disable with `stats.enabled: false` in config.

## Supported AI Tools

| AI Tool | Bash Hook | MCP Wrap | Notes |
|---|:---:|:---:|---|
| [Claude Code](https://claude.ai/code) | ✅ | ✅ | |
| [Cursor](https://cursor.sh) | ✅ | ✅ | Restart session after `gate init` to load the hook |
| [OpenCode](https://opencode.ai) | ✅ | ✅ | Restart session after `gate init` to load the hook |
| [GitHub Copilot CLI](https://github.com/features/copilot) | ✅ | ✅ | Hook is project-scoped; each developer runs `gate init` once |
| [Codex CLI](https://github.com/openai/codex) | ✅ | ✅ | After `gate init`, restart session and trust + enable the hook in the Permissions UI |
| [Gemini CLI](https://github.com/google-gemini/gemini-cli) | ✅ | ✅ | Restart session after `gate init` to load the hook |
| [CodeBuddy](https://staging-codebuddy.tencent.com/) | ✅ | ✅ | Restart session after `gate init` to load the hook |

## Supported query tools

Any command that returns JSON can be configured as a `gate` target — database clients, internal API calls via `curl`, or any other tool your AI agent uses to fetch data. The AI sees the same structured response it always did, with PII values replaced in-place.

| Command | Type | Notes |
|---|---|---|
| `tkpsql` | PostgreSQL (toolkit-managed) | `sql_arg: "--sql"` |
| `tkmsql` | MS SQL Server (toolkit-managed) | `sql_arg: "--sql"` |
| `tkdbr` | Databricks (toolkit-managed) | `sql_arg: "--sql"` |
| `databricks` | Databricks CLI (native) | `sql_arg: "--json"`, `json_sql_path: "statement"` |
| `curl` | HTTP data sources | `pipe: "jq -c ."` |
| `psql`, `mysql`, `mariadb` | Raw DB clients | **Not enabled by default** — see [Raw database clients](docs/configuration.md#raw-database-clients-opt-in) |

Prefer toolkit commands or MCP servers over raw clients: raw clients typically require credentials on the command line, which lands in the agent's transcript, shell history, and process listing. Toolkit commands ([`tk*`](https://github.com/scott-abernethy/toolkit)) inject credentials from a secrets store; MCP servers hide the connection string entirely. `gate` works with any JSON-returning command — toolkit is not required.

## Commands

```bash
gate --help                    # full subcommand list
gate <subcommand> --help       # details for any subcommand
```

The ones you'll use most:

| Command | Purpose |
|---|---|
| `gate init` | Register the hook with your harness (see Quickstart) |
| `gate export` | Write your personal config into `.gate/config.yaml` to share with the team (see Team setup) |
| `gate config` | Create and edit the YAML config; `--sync` picks up team config (see Team setup) |
| `gate scan` | PII risk report across your schema |
| `gate allowlist add/remove/list` | Manage column-name false positives |
| `gate retro` | Protection retrospective — total queries & PII fields redacted, breakdown by tool and PII type/category, hit rate with visual progress bar, and low-confidence warnings with allowlist hints |
| `gate log [-f]` | Live feed of interception events — see what gate matched, redacted, or blocked in real time. Counts and labels only, never raw command lines or PII |
| `gate enable` / `gate disable` | Toggle redaction without uninstalling |
| `gate validate` | Check config for errors before the first session; shows team config provenance when `.gate/config.yaml` is present |
| `gate protect` / `gate unprotect` *(Unix only)* | Transfer config ownership to root |
| `gate uninstall` | Remove everything gate added to your system |

See [docs/commands.md](docs/commands.md) for the full reference, including `gate run`, `gate mcp`, and the `--wrap-mcp` / `--scope` / `--harness` flags.

### Config file protection (Unix only)

For a stronger guarantee, transfer ownership of the config to root so the agent cannot modify it:

```bash
sudo gate protect      # any future enable/disable/config/allowlist now needs sudo
sudo gate unprotect    # restore direct write access
```

Enforced at the OS level across all harnesses (Claude Code, OpenCode, Cursor, GitHub Copilot CLI, Codex CLI, Gemini CLI, CodeBuddy). Not supported on Windows.

## Documentation

- [Configuration](docs/configuration.md) — full YAML schema and built-in PII detection rules
- [Team configuration](docs/team.md) — sharing config across a team via `.gate/config.yaml`
- [Commands](docs/commands.md) — full subcommand reference
- [MCP setup](docs/mcp.md) — wrapping existing MCP servers and registering new ones
- [Scan queries](docs/scan.md) — schema-query examples for each database
- [Config file locations](docs/config-locations.md) — where each harness stores hooks and MCP settings
- [Troubleshooting](docs/troubleshooting.md) — common issues and fixes

## Uninstallation

```bash
gate uninstall
brew uninstall gate
```

`gate uninstall` removes gate hooks from all harnesses, the config directory at `~/.config/gate/`, and any gate-generated plugin files. It shows what will be deleted and asks for confirmation.

## Contributing

Bug reports and pull requests are welcome. For significant changes, open an issue first to discuss the proposal. See [CONTRIBUTING.md](CONTRIBUTING.md) for the dev setup, pre-commit checklist, and safety rules for redaction changes.

## License

MIT — see [LICENSE](LICENSE).

## Disclaimer

See [DISCLAIMER.md](DISCLAIMER.md).
