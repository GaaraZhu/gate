# Config file locations

## Gate's own config

| Scope | Location |
|---|---|
| Personal (override with `GATE_CONFIG`) | `~/.config/gate/config.yaml` |
| Project, git-tracked (override with `GATE_PROJECT_CONFIG`) | `.gate/config.yaml` at the repo root |

The project file, when present, is merged into the personal config — see [docs/team.md](team.md) for the merge rules, `gate export`, and `gate config --sync`.

## Hook settings

| Harness | Global / user | Project |
|---|---|---|
| Claude Code | `~/.claude/settings.json` | `.claude/settings.json` |
| OpenCode | `~/.config/opencode/opencode.json` | `./opencode.json` |
| Cursor | `~/.cursor/hooks.json` | `.cursor/hooks.json` |
| Copilot CLI | — (not supported) | `.github/hooks/PreToolUse.json` |
| Codex CLI | `~/.codex/hooks.json` | `.codex/hooks.json` |
| Gemini CLI | `~/.gemini/settings.json` | `.gemini/settings.json` |
| CodeBuddy | `~/.codebuddy/settings.json` | `.codebuddy/settings.json` |

## MCP server config

| Harness | Global / user | Project |
|---|---|---|
| Claude Code | `~/.claude.json` | `./.mcp.json` |
| OpenCode | `~/.config/opencode/opencode.json` | `./opencode.json` |
| Cursor | `~/.cursor/mcp.json` | `.cursor/mcp.json` |
| Copilot CLI | `~/.copilot/mcp-config.json` | `./.mcp.json` |
| Codex CLI | `~/.codex/config.toml` | `.codex/config.toml` |
| Gemini CLI | `~/.gemini/settings.json` | `.gemini/settings.json` |
| CodeBuddy | `~/.codebuddy/settings.json` | `.codebuddy/settings.json` |

OpenCode, Gemini CLI, and CodeBuddy store both hooks and MCP servers in the same file. Claude Code, Cursor, and Copilot CLI use separate files for each. Codex CLI uses a TOML config file for MCP servers.
