# Team configuration

Each developer normally runs `gate` with their own personal config at `~/.config/gate/config.yaml`. On a team this creates drift: one person tightens a threshold or adds a detection pattern, nobody else gets it unless they remember to copy it over by hand.

`gate` closes that gap with a project-scoped config file, `.gate/config.yaml`, committed to git and shared by everyone who clones the repo.

## The file: `.gate/config.yaml`

```
repo/
  .gate/
    config.yaml     # team-owned, committed to git
```

It's a **restricted** config — only a subset of fields, and every field is either purely additive or explicitly called out as an exception:

| Field | Merge rule |
|---|---|
| `min_gate_version` | Warns (does not block) if the installed `gate` is older |
| `tools` | Union — adds tools, project entries win on a name collision |
| `pii.patterns` | Union — adds patterns, project entries win on a name collision |
| `pii.column_denylist` | Union — adds forced-redact columns |
| `pii.confidence_threshold` | Whichever value is **higher** wins |
| `pii.column_name_boost` | Whichever value is **higher** wins |
| `pii.column_allowlist` | Union — see the warning below |

`.gate/config.yaml` **cannot** set `enabled`, `action`, `wildcard_policy`, the redaction template, hashing, or `mcp`/`stats` settings — those fields don't exist in the project config schema at all, so there's no way to express them there even by mistake.

### The one exception: `column_allowlist`

Every other field can only make protection stricter. `column_allowlist` is different — an entry there tells gate to skip redaction for that column name entirely. Union-merging it means a project config *can* reduce redaction for the whole team. `gate` doesn't hide this:

- `gate export` prints a note listing any allowlist entries it wrote.
- `gate config --sync`, when it merges an allowlist entry into your personal config, says so explicitly.
- `gate validate` shows project-sourced allowlist entries in its provenance output.

Review allowlist entries in `.gate/config.yaml` the same way you'd review any other change to a shared file before committing it.

## Setting it up (team lead, once)

```bash
gate export
```

This writes your current personal config's tools, patterns, thresholds, `column_denylist`, and `column_allowlist` into `.gate/config.yaml` (fields outside that list — `enabled`, `action`, hashing, etc. — stay personal and are never written). Review the file, then commit it:

```bash
git add .gate/config.yaml
git commit -m "add gate team config"
```

`gate export` always overwrites `.gate/config.yaml` if it already exists — there's no `--force` to remember. It's a git-tracked file, so commit before re-running if you want a rollback point; an uncommitted overwrite isn't recoverable.

You can hand-edit `.gate/config.yaml` directly instead of (or after) exporting — it's just YAML matching the schema above.

## Picking it up (everyone else)

```bash
git clone <repo>
gate init            # installs the harness hook
gate config --sync   # scaffolds/picks up .gate/config.yaml, merges it into personal config
```

`gate init` only ever touches harness hook registration. `gate config --sync` is the command for `.gate/config.yaml` itself, and does two things:

1. **Ensures `.gate/config.yaml` exists.** If the repo doesn't have one yet, it writes a blank, commented starter. If one already exists, it's left alone — team config, once committed, is edited by hand and reviewed like any other file in the repo.
2. **Merges it into your personal config.** The fields above are merged into `~/.config/gate/config.yaml` on disk, using the same rules as the table above (a project pattern is added, a higher project threshold wins, etc.). This is additive and idempotent — running `gate config --sync` again with no changes to `.gate/config.yaml` does nothing and prints nothing.

`--sync` is non-interactive (unlike plain `gate config`, which opens an editor) and safe to run inside an agent harness — same safety profile as `gate config --init-only`.

Re-run `gate config --sync` any time `.gate/config.yaml` changes (after a `git pull`) to pick up the update.

### Why merge into personal config instead of just reading the project file live?

`gate` *also* merges `.gate/config.yaml` in-memory on every invocation, purely by walking up from the current directory (the same way git finds `.git`) — so as long as your shell is inside the project when a command runs, you get the merged rules automatically, with no `gate config --sync` needed.

The persistent merge exists for the gap that leaves: `gate hook`/`gate run` inherit whatever directory your shell is actually in at that moment. If an agent session `cd`s elsewhere mid-session — into a scratch directory, a different checkout, wherever — and then runs a query, the live directory walk won't find `.gate/config.yaml`, and protection silently falls back to whatever's in personal config alone. Baking the safe fields into personal config once (via `gate config --sync`) means they apply everywhere afterward, independent of the shell's current directory.

This is why `column_allowlist` is included in the personal merge despite the tradeoff described above: once merged, an allowlist entry from this project applies to *every* project you use `gate` on, not just this one. That's a deliberate, explicit choice — review allowlist entries in `.gate/config.yaml` before committing them, the same as any other config change with team-wide reach.

## Checking what's active: `gate validate`

```bash
gate validate
```

When a project config is in play, `gate validate` reports where the effective config came from:

```
Config is valid.
  Source: project (.gate/config.yaml) + user (~/.config/gate/config.yaml)
  Effective confidence_threshold: 0.8 (project, overrides user 0.5)
  Effective tools: bq, psql, tkpsql (2 from project, 1 personal)
  Project column_allowlist adds 1 entries (reduces redaction team-wide): employee_id
```

It also warns if:
- the installed `gate` is older than `.gate/config.yaml`'s `min_gate_version`, or
- `.gate/config.yaml` exists but fails to parse — in which case gate falls back to personal config alone rather than failing every command. A broken team file should never silently disable everyone's protection.

## What this does NOT include

- **Credentials in project config.** Database creds stay personal (`~/.pgpass`, `.env`, secrets managers) — never put them in `.gate/config.yaml`.
- **A sync daemon or auto-push/pull.** Git is the distribution mechanism. `gate export` writes the file; `git commit`/`git push`/`git pull` move it around; `gate config --sync` picks it up.
- **A central server.** Each developer runs their own local `gate`; there's no shared runtime state.
