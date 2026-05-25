## [0.8.7] - 2026-05-25

### 🚀 Features

- Add Codex CLI bash hook support (Part A)
- Add Codex CLI MCP wrap support (Part B)

### 📚 Documentation

- Add Codex CLI to documentation

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.6
## [0.8.6] - 2026-05-23

### 🚀 Features

- Refactor --harness and --format to ValueEnum, update docs for cursor MCP
- Add Cursor hook detection to gate validate
- Add MCP wrap detection to gate validate
- Split gate validate output into Bash hooks and MCP wraps sections

### 📚 Documentation

- Update gate init and hook help text for cursor support
- Add Supported AI Tools table, cursor MCP notes, and cleanup

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.5
## [0.8.5] - 2026-05-23

### 🚀 Features

- Add Cursor MCP wrap and registration support

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.4
## [0.8.4] - 2026-05-23

### 🚀 Features

- Add Cursor harness support

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.3
## [0.8.3] - 2026-05-22

### 🚀 Features

- Add AU/NZ PII column synonyms
- Rename column_names to column_denylist, add AU/NZ PII coverage and EAV docs

### 🐛 Bug Fixes

- Disable ANSI colors in cmd.exe, enable VT processing on Windows Terminal
- Extract color detection to shared module, apply to retro output
- Remove needless return flagged by clippy on Windows target

### 📚 Documentation

- Add sqlcmd support and Windows pipe instructions to scan docs

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.2
## [0.8.2] - 2026-05-21

### 🚀 Features

- Add sqlcmd fixed-width table format support to gate scan

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.1
## [0.8.1] - 2026-05-21

### 🐛 Bug Fixes

- Normalize CRLF line endings before YAML parsing on Windows

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.0
## [0.8.0] - 2026-05-20

### 🚀 Features

- Show Copilot CLI in gate validate output

### 🐛 Bug Fixes

- Add project level harness integrations in validate output

### 📚 Documentation

- Adds security review to readme

### ⚙️ Miscellaneous Tasks

- Bump version to 0.7.2
## [0.7.2] - 2026-05-19

### 🚀 Features

- Adds category and sub category to retro output

### 🐛 Bug Fixes

- Synchronize environment variable mocking in init tests

### 📚 Documentation

- Update readme with latest retro command state
- Update readme with retro command

### ⚙️ Miscellaneous Tasks

- Bump version to 0.7.1
## [0.7.1] - 2026-05-18

### 🚀 Features

- Adds tool breakdown to retro output

### 🐛 Bug Fixes

- Clarify error message in gate protect/unprotect
- Align multi-line command descriptions in help text
- Correct alignment of multi-line command descriptions in help

### 🚜 Refactor

- Reorder commands in help output and readme

### ⚙️ Miscellaneous Tasks

- Bump version to 0.7.0
## [0.7.0] - 2026-05-18

### 🚀 Features

- *(retro)* Add hit rate, top categories, and improve output polish

### 🐛 Bug Fixes

- Move config_path import inside cfg(unix) blocks to fix Windows build

### 📚 Documentation

- Simplify README and split overflow into docs/

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.12
## [0.6.12] - 2026-05-18

### 🚀 Features

- Disallow disable/enable gate from agent
- Disallow agent from updating gate config (unix only)

### 📚 Documentation

- Make raw clients opt-in

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.10
- Bump version to 0.6.11
## [0.6.10] - 2026-05-18

### 🐛 Bug Fixes

- Updates dev database
- Solves small issues (windows path, default editor, pass through log etc)

### 📚 Documentation

- Adds demo gif
- Simplify scan schema section in readme
- Updates demo.gif
- Updates readme

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.9
- Clean ups — CHANGELOG, clippy hygiene, docs
## [0.6.9] - 2026-05-16

### 🐛 Bug Fixes

- Consistent alignment in scan output table
- Respect global enablement config in mcp

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.8
## [0.6.8] - 2026-05-16

### 🚜 Refactor

- Update dev dataset
- Improves scan output

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.7
## [0.6.7] - 2026-05-15

### 🚀 Features

- *(scan)* Hide Detected Categories and Top Findings when no PII found
- *(scan)* Show NONE risk level when no PII columns detected

### 🐛 Bug Fixes

- Use bright green for NONE risk

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.6
- Explicitly add ~/.cargo/bin to PATH before build step
- Remove rust-cache and use full cargo path on release builds
- Use full cargo path to fix macos-14 rustup-init conflict
## [0.6.6] - 2026-05-14

### 🚀 Features

- *(scan)* Native support for Databricks API response format

### 🐛 Bug Fixes

- Remove suburb, city, state, province, country from PII list

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.5
## [0.6.5] - 2026-05-14

### 🚀 Features

- Gate scan --source and --diff for team-shared change detection
- Add gate config sync for team-shared PII column classifications
- *(scan)* Improve report visual design
- *(scan)* Highlight section headers in bold bright-cyan
- *(scan)* Drop redundant sensitivity label from Top Findings
- *(databricks)* Add support for Databricks CLI with JSON SQL extraction

### 🐛 Bug Fixes

- Address all three pre-launch blockers
- *(mcp)* Fail-closed on oversized tools/call payloads
- Propagate stdin read errors in redact_stdin
- *(scan)* Use column(s) to handle singular count correctly

### 📚 Documentation

- Add SECURITY.md, CONTRIBUTING.md, and CHANGELOG.md
- Disclose MCP interception scope in README
- Document Databricks CLI support in README
- Uses consistent case for scan schema queries

### 🧪 Testing

- Rename DB user redact → gate_demo in run_integration fixture

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.4
- Add cargo audit advisory check
- *(dev)* Fix README schema and remove api-server
## [0.6.4] - 2026-05-13

### 🚀 Features

- Add column allowlist to skip name-based PII redaction
- Gate scan now accepts CSV input from GUI database clients

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.3
## [0.6.3] - 2026-05-12

### 🐛 Bug Fixes

- Gate scan --verbose now shows all categories, not just top 3

### 📚 Documentation

- Add threat model document

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.2
## [0.6.2] - 2026-05-12

### 🚀 Features

- Add project-scope support for Claude Code hook install

### 🐛 Bug Fixes

- Replace --clobber with delete-then-create for gh release
- Always build from main so version matches the tag
- Skip run_integration tests on Windows
- Skip mcp_integration tests on Windows
- Gate editor_invoked_via_editor_env test to unix only

### 📚 Documentation

- Add config file locations reference tables
- Remove duplicate MCP setup commands from How it works section
- Update MCP example to show actual tools/call response format
- Update readme
- Update demo screenshots
- Update demo screenshots

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.1
## [0.6.1] - 2026-05-12

### 🚀 Features

- Add workflow_dispatch trigger to release workflow

### 🐛 Bug Fixes

- Pass --repo to gh release create to avoid missing git context

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.0
## [0.6.0] - 2026-05-12

### 🚀 Features

- Add project level mcp support
- Add gate init --wrap-mcp to bulk-convert existing MCP servers to proxies
- Add cross-platform release artifacts and multi-arch Homebrew formula
- Add GitHub Copilot CLI support (bash hook + MCP)

### 🐛 Bug Fixes

- Align opencode MCP config with actual opencode.json schema

### 📚 Documentation

- Rename local mcp name
- Update readme to match latest project state
- Update Claude.md

### ⚙️ Miscellaneous Tasks

- Bump version to 0.5.1
## [0.5.1] - 2026-05-11

### 🐛 Bug Fixes

- Gate init --mcp writes to ~/.claude.json for claude-code

### ⚙️ Miscellaneous Tasks

- Bump version to 0.5.0
## [0.5.0] - 2026-05-11

### 🚀 Features

- Add gate mcp stdio proxy with PII redaction
- Complete MCP proxy implementation plan gaps

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.9
## [0.4.9] - 2026-05-10

### 🚀 Features

- Weight scan risk level by category sensitivity
- Add --json output mode to gate scan

### 🐛 Bug Fixes

- Surface id-typed PII columns in scan breakdown

### 🚜 Refactor

- Move map_to_tier1_category to patterns.rs

### 📚 Documentation

- Updates readme
- Add scan coverage disclaimer to report and README
- Update README banner image
- Update README demo image

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.8
## [0.4.8] - 2026-05-09

### 🚀 Features

- Add --verbose flag to scan command

### 📚 Documentation

- Remove stale references to /docs folder
- Simplify status section, remove milestone checklist

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.7
## [0.4.7] - 2026-05-08

### 🐛 Bug Fixes

- Supports psql resp format in scan

### 🚜 Refactor

- Updates scan output
- Scan output rework

### 📚 Documentation

- Update README banner image
- Support psql output in scan

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.6
## [0.4.6] - 2026-05-08

### 🚀 Features

- Makes scan stdin pipeable

### 🐛 Bug Fixes

- Fmt format

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.4
- Bump version to 0.4.5
## [0.4.5] - 2026-05-08

### 🚀 Features

- *(dev)* Adds MCP for local database
- Support auditing database for PII exposure

### 📚 Documentation

- Remove login from PII synonyms table and add non-JSON verbose troubleshooting entry
- Update readme
- Update readme for MCP support plan

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.3
## [0.4.3] - 2026-05-07

### 🐛 Bug Fixes

- Remove double-pipe in hook — let gate run apply pipe once

### 📚 Documentation

- Replace opencode screenshot with text note in demo section

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.2
## [0.4.2] - 2026-05-07

### 🐛 Bug Fixes

- Suppress login column match on _at timestamp suffix (last_login_at false positive)

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.1
- Remove login from PII token synonyms
## [0.4.1] - 2026-05-07

### 🚀 Features

- Inject -s into curl to suppress progress output in gate run

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.0
## [0.4.0] - 2026-05-07

### 🚀 Features

- Adds psql support
- Detect columnar JSON by key aliases (headers/records, keys/results, fields/data)
- Expand built-in PII detection to cover 14 categories

### 🐛 Bug Fixes

- Three false positives in gate's PII redactor
- Redact PII from verbose logs and add account_name as name

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.8
- Bump version to 0.3.9
## [0.3.8] - 2026-05-07

### 🚀 Features

- Adds salutation to PII list

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.7
## [0.3.7] - 2026-05-07

### 🐛 Bug Fixes

- Supports columnar type json
- Cargo fmt failure

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.5
- Bump version to 0.3.6
## [0.3.6] - 2026-05-07

### 🐛 Bug Fixes

- Tighten error-shape detection to non-empty string values only

### 📚 Documentation

- Update README for curl, direct DB clients, --verbose, and stdin mode

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.4
## [0.3.4] - 2026-05-07

### 🚀 Features

- Add --verbose flag to gate run for redaction debugging
- Gate run reads JSON from stdin when called with no args

### 🐛 Bug Fixes

- Gate hook reads chained input from hookSpecificOutput.updatedInput

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.3
- Add dev/api-server helper script for local demo
## [0.3.3] - 2026-05-06

### 🚀 Features

- Supports curl command

### 🐛 Bug Fixes

- Commit Cargo.lock in release pipeline version bump

### 📚 Documentation

- Add uninstall instructions to README
- Improve README structure and content
- Adds security scope
- Moves uninstallation after output format

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.2
- Updates screenshots
## [0.3.2] - 2026-05-06

### 🚀 Features

- Add gate uninstall subcommand
- Show pending actions and confirm before uninstalling

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.1
## [0.3.1] - 2026-05-06

### 📚 Documentation

- Document hash_values/hash_salt in README and starter config

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.0
## [0.3.0] - 2026-05-06

### 🚀 Features

- Deterministic hashing for redacted PII values

### 🐛 Bug Fixes

- Corrects more references
- Updates banner

### 📚 Documentation

- Add banner, badges, LICENSE, DISCLAIMER, and move screenshots to assets
- Fix remaining redact references and update tagline in README

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.1
- Remove internal docs from tracking and add docs/ to .gitignore
- Rename project from redact to gate
## [0.2.1] - 2026-05-05

### 🚀 Features

- Expand Gate 1 name-column coverage for surname, given/family name, plurals, and person prefixes

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.0
- Add Homebrew formula auto-update to release workflow and expand seed data
## [0.2.0] - 2026-05-04

### 📚 Documentation

- Add Claude Code demo, add tkmsql, drop raw-client placeholders

### ⚙️ Miscellaneous Tasks

- Bump version to 0.1.6
## [0.1.6] - 2026-05-04

### 🚀 Features

- Implement Milestone 8 — opencode support

### 🐛 Bug Fixes

- Apply cargo fmt to validate.rs

### 📚 Documentation

- Spec Milestone 8 (Copilot CLI, deferred) and Milestone 9 (opencode)
- Promote opencode to Milestone 8, demote Copilot CLI to Milestone 9
- Promote opencode to supported harness in README

### ⚙️ Miscellaneous Tasks

- Bump version to 0.1.5
## [0.1.5] - 2026-05-03

### 🐛 Bug Fixes

- Honour enabled flag and REDACT_DISABLED in redact run

### 📚 Documentation

- Fix incorrect links

### ⚙️ Miscellaneous Tasks

- Bump version to 0.1.4
## [0.1.4] - 2026-05-03

### 🚀 Features

- Add redact enable/disable and REDACT_DISABLED env var

### 📚 Documentation

- Update README with current JSON-tool support and psql roadmap
- Add enabled flag and REDACT_DISABLED to README config example

### ⚙️ Miscellaneous Tasks

- Bump version to 0.1.3
## [0.1.3] - 2026-05-03

### ⚙️ Miscellaneous Tasks

- Auto-bump Cargo.toml version from tag in release workflow
## [0.1.2] - 2026-05-03

### 🚀 Features

- Change wildcard_policy default from reject to warn, add Milestone 7 plan
## [0.1.1] - 2026-05-03

### 🚀 Features

- *(hook)* Intercept tools nested in sh -c and kubectl exec --

### 📚 Documentation

- Add Homebrew install instructions to README

### ⚙️ Miscellaneous Tasks

- Add release workflow for aarch64-apple-darwin binary
- Fix YAML syntax error in release workflow
## [0.1.0] - 2026-05-03

### 🚀 Features

- *(redact)* Add hook, run, and PII redactor
- *(common)* Implement Milestone 1 foundation
- *(common)* Implement Milestone 2 Gate 2 redactor
- *(gate1)* Implement Milestone 3 SQL tokenizer and column extractor
- *(redact)* Implement Milestone 4 — redact run pipeline
- *(redact)* Implement Milestone 5 — hook surface
- *(redact)* Implement Milestone 6 — polish & ship
- *(dev)* Add local demo environment for end-to-end testing
- *(hook)* Auto-rewrite raw DB CLI commands to JSON-output wrappers
- *(run)* Resolve json_tool wrapper and translate sql_arg flag
- *(hook,run)* Intercept tool invocations regardless of wrapper prefix count

### 🐛 Bug Fixes

- *(dev)* Cd into dev/ for docker compose to avoid -f flag issue
- *(run)* Handle KEY=VALUE env-var prefix in redact run subprocess spawning

### 📚 Documentation

- Add initial requirements, design, and plan
- Add README with overview, installation, configuration, and security model
- Add CLAUDE.md with project context, build commands, and invariants
- Add prototype section to plan and update CLAUDE.md current step
- Adds demo screenshot
- Move Demo section above How it works in README
- Fix Demo section wording to match screenshot
- Update How it works diagram to match demo use case
- Fix README inaccuracies found in audit

### 🎨 Styling

- Apply rustfmt to Milestone 1 test files
- Apply cargo fmt

### 🧪 Testing

- *(common)* Add Milestone 1 unit tests (Step 8)

### ⚙️ Miscellaneous Tasks

- Add .gitignore for Rust binary, editor files, and .claude/
- Complete repository setup (Steps 1-3)
- Update CLAUDE.md for Milestone 2 and refine common patterns
- Update CLAUDE.md to Milestone 3
- Add pre-commit checks to CLAUDE.md and fix cargo fmt
