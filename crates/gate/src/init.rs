use common::error::exit_with_error;
use common::harness::is_agent_harness;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const HOOK_COMMAND: &str = "gate hook";
const COPILOT_HOOK_COMMAND: &str = "gate hook --format copilot";
const CURSOR_HOOK_COMMAND: &str = "gate hook --format cursor";
const CODEX_HOOK_COMMAND: &str = "gate hook --format codex";
const GEMINI_HOOK_COMMAND: &str = "gate hook --format gemini";
const CODEBUDDY_HOOK_COMMAND: &str = "gate hook --format codebuddy";

pub fn run(
    harness: &str,
    scope: &str,
    mcp: Option<&str>,
    mcp_cmd: Option<&str>,
    wrap_mcp: bool,
    servers: Option<&str>,
    yes: bool,
) {
    if is_agent_harness() {
        exit_with_error(
            "gate init is not available inside an agent harness. \
             Run `gate init` in a terminal session outside the agent.",
        );
    }

    if mcp.is_some() && wrap_mcp {
        exit_with_error("--mcp and --wrap-mcp cannot be used together");
    }

    if servers.is_some() && !wrap_mcp {
        exit_with_error("--servers requires --wrap-mcp");
    }

    let filter = parse_servers_filter(servers);

    if wrap_mcp {
        match harness {
            "claude-code" => {
                let path = match claude_code_mcp_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                wrap_mcp_claude(&path, filter.as_deref(), yes);
            }
            "opencode" => {
                let path = match opencode_config_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&format!("cannot resolve settings path: {e}")),
                };
                wrap_mcp_opencode(&path, filter.as_deref(), yes);
            }
            "copilot-cli" => {
                let path = match copilot_mcp_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                wrap_mcp_claude(&path, filter.as_deref(), yes);
            }
            "cursor" => {
                let path = match cursor_mcp_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                wrap_mcp_claude(&path, filter.as_deref(), yes);
            }
            "codex" => {
                let path = match codex_config_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                wrap_mcp_codex(&path, filter.as_deref(), yes);
            }
            "gemini" => {
                let path = match gemini_settings_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                wrap_mcp_claude(&path, filter.as_deref(), yes);
            }
            "codebuddy" => {
                let path = match codebuddy_settings_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                wrap_mcp_claude(&path, filter.as_deref(), yes);
            }
            _ => exit_with_error(&format!(
                "--wrap-mcp is not supported for harness '{harness}'; \
                 supported: claude-code, opencode, copilot-cli, cursor, codex, gemini, codebuddy"
            )),
        }
        return;
    }

    if let Some(server_name) = mcp {
        let cmd_str = mcp_cmd.unwrap_or_else(|| {
            exit_with_error(
                "--mcp-cmd is required when --mcp is set. \
                Example: gate init --mcp postgres --mcp-cmd \"uvx mcp-server-postgres\"",
            )
        });
        match harness {
            "claude-code" => {
                let path = match claude_code_mcp_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                register_mcp_server(&path, server_name, cmd_str);
            }
            "opencode" => {
                let path = match opencode_config_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&format!("cannot resolve settings path: {e}")),
                };
                register_mcp_server_opencode(&path, server_name, cmd_str);
            }
            "copilot-cli" => {
                let path = match copilot_mcp_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                register_mcp_server(&path, server_name, cmd_str);
            }
            "cursor" => {
                let path = match cursor_mcp_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                register_mcp_server(&path, server_name, cmd_str);
            }
            "codex" => {
                let path = match codex_config_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                register_mcp_server_codex(&path, server_name, cmd_str);
            }
            "gemini" => {
                let path = match gemini_settings_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                register_mcp_server(&path, server_name, cmd_str);
            }
            "codebuddy" => {
                let path = match codebuddy_settings_path(scope) {
                    Ok(p) => p,
                    Err(e) => exit_with_error(&e),
                };
                register_mcp_server(&path, server_name, cmd_str);
            }
            _ => exit_with_error(&format!(
                "MCP registration is not supported for harness '{harness}'; \
                 supported: claude-code, opencode, copilot-cli, cursor, codex, gemini, codebuddy"
            )),
        }
        return;
    }

    match harness {
        "claude-code" => {
            let path = match claude_settings_path(scope) {
                Ok(p) => p,
                Err(e) => exit_with_error(&format!("cannot resolve settings path: {e}")),
            };
            run_with_path(&path);
        }
        "opencode" => crate::init_opencode::run(scope),
        "copilot-cli" => init_copilot_cli(scope),
        "cursor" => init_cursor(scope),
        "codex" => init_codex(scope),
        "gemini" => init_gemini(scope),
        "codebuddy" => init_codebuddy(scope),
        _ => exit_with_error(&format!(
            "unsupported harness '{harness}'; supported: claude-code, opencode, copilot-cli, cursor, codex, gemini, codebuddy. \
             Usage: gate init --harness <harness>"
        )),
    }
}

fn run_with_path(path: &Path) {
    let settings = read_settings(path);
    match insert_hook(settings) {
        HookInsertResult::AlreadyInstalled => {
            println!("gate hook is already installed in {}", path.display());
        }
        HookInsertResult::Done(updated) => {
            write_atomic(path, &updated).unwrap_or_else(|e| {
                exit_with_error(&format!("failed to write {}: {e}", path.display()))
            });
            println!("gate hook installed in {}", path.display());
            println!("Run `gate config` to define which tools to intercept.");
        }
    }
}

enum HookInsertResult {
    AlreadyInstalled,
    Done(Value),
}

fn read_settings(path: &Path) -> Value {
    if !path.exists() {
        return json!({});
    }
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to read {}: {e}", path.display())));
    serde_json::from_str(&contents)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to parse {}: {e}", path.display())))
}

fn new_hook_entry() -> Value {
    json!({
        "matcher": "Bash",
        "hooks": [{ "type": "command", "command": HOOK_COMMAND }]
    })
}

fn insert_hook(mut settings: Value) -> HookInsertResult {
    normalize_settings(&mut settings);

    // Check for exact match (already installed)
    let already = {
        let arr = settings["hooks"]["PreToolUse"].as_array().unwrap();
        has_exact_hook(arr)
    };
    if already {
        return HookInsertResult::AlreadyInstalled;
    }

    // Remove any gate hook variants, then append the canonical entry
    {
        let arr = settings["hooks"]["PreToolUse"].as_array_mut().unwrap();
        arr.retain(|entry| !entry_has_gate_hook(entry));
        arr.push(new_hook_entry());
    }

    HookInsertResult::Done(settings)
}

/// Ensure `settings` is `{"hooks": {"PreToolUse": [...]}}` (creating missing layers).
fn normalize_settings(settings: &mut Value) {
    if !settings.is_object() {
        *settings = json!({});
    }
    let obj = settings.as_object_mut().unwrap();

    let hooks = obj.entry("hooks".to_string()).or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }

    let pretu = hooks
        .as_object_mut()
        .unwrap()
        .entry("PreToolUse".to_string())
        .or_insert_with(|| json!([]));
    if !pretu.is_array() {
        *pretu = json!([]);
    }
}

fn has_exact_hook(arr: &[Value]) -> bool {
    arr.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks
                    .iter()
                    .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(HOOK_COMMAND))
            })
            .unwrap_or(false)
    })
}

pub(crate) fn entry_has_gate_hook(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(is_gate_hook_variant)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Matches `gate hook` and variants like `/usr/local/bin/gate hook`.
fn is_gate_hook_variant(cmd: &str) -> bool {
    let mut parts = cmd.splitn(2, ' ');
    let prog = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim_start();
    let basename = Path::new(prog)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(prog);
    basename == "gate" && rest.starts_with("hook")
}

fn write_atomic(path: &Path, value: &Value) -> anyhow::Result<()> {
    let json_str = serde_json::to_string_pretty(value)?;
    write_text_atomic(path, &json_str)
}

fn write_text_atomic(path: &Path, contents: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("settings path has no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("settings path has no filename"))?;
    let tmp_path = parent.join(format!("{file_name}.gate_tmp"));
    std::fs::write(&tmp_path, contents)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// `gate config --sync`'s source-discovery step: walks up from `start_dir`
/// looking for a team config, checking each directory for `.gate/config.yaml`
/// then a bare `config.yaml` (mirrors the walk `project_config_path` does for
/// the live runtime merge, plus the bare-file fallback). Returns `None` if
/// neither is found before reaching the filesystem root.
fn find_team_config_source(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let nested = dir.join(".gate").join("config.yaml");
        if nested.exists() {
            return Some(nested);
        }
        let bare = dir.join("config.yaml");
        if bare.exists() {
            return Some(bare);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// `gate config --sync`: finds a team config above (or in) `start_dir` and
/// merges it into personal config. No git repository required. Never creates
/// a team config — that's `gate export`'s job; `--sync` only picks up what's
/// already there.
pub(crate) fn sync_team_config(start_dir: &Path) {
    let Some(team_path) = find_team_config_source(start_dir) else {
        println!(
            "No team config found (.gate/config.yaml or config.yaml, searched from {} upward). \
             Run `gate export` to create one from your personal config.",
            start_dir.display()
        );
        return;
    };
    merge_project_into_personal(&team_path);
}

/// `gate config --sync`'s personal-merge step: reads the team config at
/// `team_path` and merges it into the caller's personal config file on disk,
/// using the same tighten-only rules as the in-memory runtime merge (including
/// `column_allowlist`, per an explicit choice to accept that a project's
/// allowlist entries become permanent and global once merged — see
/// `Config::apply_project`'s doc comment for the tradeoff). Idempotent: only
/// writes when something actually changed, and only reports what changed.
///
/// This rewrites the personal config file wholesale (full struct round-trip via
/// serde_yaml), which does not preserve hand-added comments or formatting —
/// an accepted tradeoff for guaranteeing the merge survives regardless of the
/// caller's shell CWD later (unlike the in-memory merge, which only applies
/// while CWD is inside this repo).
fn merge_project_into_personal(team_path: &Path) {
    let project = match common::config::ProjectConfig::load_from_path(team_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "gate: failed to parse {} ({e}) — skipping personal config merge",
                team_path.display()
            );
            return;
        }
    };

    let personal_path = match common::config::config_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "gate: failed to resolve personal config path ({e}) — skipping personal config merge"
            );
            return;
        }
    };
    let mut personal = common::config::Config::load_from_path(&personal_path)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to load personal config: {e}")));

    let threshold_before = personal.pii.confidence_threshold;
    let boost_before = personal.pii.column_name_boost;
    let tools_before: std::collections::HashSet<String> = personal.tools.keys().cloned().collect();
    let patterns_before: std::collections::HashSet<String> =
        personal.pii.patterns.keys().cloned().collect();
    let denylist_before: std::collections::HashSet<String> =
        personal.pii.column_denylist.iter().cloned().collect();
    let allowlist_before: std::collections::HashSet<String> =
        personal.pii.column_allowlist.iter().cloned().collect();

    personal.apply_project(&project);

    let threshold_changed = personal.pii.confidence_threshold != threshold_before;
    let boost_changed = personal.pii.column_name_boost != boost_before;
    let new = |before: &std::collections::HashSet<String>, after: &[String]| -> Vec<String> {
        let mut v: Vec<String> = after
            .iter()
            .filter(|s| !before.contains(*s))
            .cloned()
            .collect();
        v.sort();
        v
    };
    let mut new_tools: Vec<String> = personal
        .tools
        .keys()
        .filter(|t| !tools_before.contains(*t))
        .cloned()
        .collect();
    new_tools.sort();
    let new_patterns_list: Vec<String> = {
        let mut v: Vec<String> = personal
            .pii
            .patterns
            .keys()
            .filter(|p| !patterns_before.contains(*p))
            .cloned()
            .collect();
        v.sort();
        v
    };
    let new_denylist = new(&denylist_before, &personal.pii.column_denylist);
    let new_allowlist = new(&allowlist_before, &personal.pii.column_allowlist);

    let changed = threshold_changed
        || boost_changed
        || !new_tools.is_empty()
        || !new_patterns_list.is_empty()
        || !new_denylist.is_empty()
        || !new_allowlist.is_empty();
    if !changed {
        return;
    }

    let yaml = serde_yaml::to_string(&personal)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to serialize personal config: {e}")));
    write_text_atomic(&personal_path, &yaml)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to write personal config: {e}")));

    println!(
        "Merged {} into personal config at {}",
        team_path.display(),
        personal_path.display()
    );
    if threshold_changed {
        println!(
            "  confidence_threshold: {threshold_before} -> {}",
            personal.pii.confidence_threshold
        );
    }
    if boost_changed {
        println!(
            "  column_name_boost: {boost_before} -> {}",
            personal.pii.column_name_boost
        );
    }
    if !new_tools.is_empty() {
        println!("  New tools: {}", new_tools.join(", "));
    }
    if !new_patterns_list.is_empty() {
        println!("  New patterns: {}", new_patterns_list.join(", "));
    }
    if !new_denylist.is_empty() {
        println!("  New column_denylist entries: {}", new_denylist.join(", "));
    }
    if !new_allowlist.is_empty() {
        println!(
            "  New column_allowlist entries — now applies everywhere you use gate, not just this project: {}",
            new_allowlist.join(", ")
        );
    }
}

/// `gate export`: writes the caller's personal config into `config.yaml` in
/// the current directory so it can be committed and shared with the team. No
/// git repository required. Always overwrites — this is meant to be a
/// git-tracked file, so an unwanted overwrite is a `git checkout` away as long
/// as it's committed.
pub fn run_export() {
    let cwd = std::env::current_dir()
        .unwrap_or_else(|e| exit_with_error(&format!("failed to resolve current directory: {e}")));

    let personal = common::config::Config::load()
        .unwrap_or_else(|e| exit_with_error(&format!("failed to load personal config: {e}")));
    write_team_config_from_personal(&cwd.join("config.yaml"), &personal);
}

/// Exports `personal` into `path`, overwriting whatever is there. Only fields
/// `ProjectConfig` can express are carried over (tools, patterns, thresholds,
/// column_denylist, column_allowlist) — `enabled`, `action`, `wildcard_policy`,
/// redaction format, hashing, and mcp/stats settings stay personal, since
/// they're either local/stylistic or (for `enabled`) not something a project
/// file can set at all. Callers decide whether overwriting is appropriate.
fn write_team_config_from_personal(path: &Path, personal: &common::config::Config) {
    let project = common::config::ProjectConfig {
        min_gate_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        tools: personal.tools.clone(),
        pii: common::config::ProjectPiiConfig {
            patterns: personal.pii.patterns.clone(),
            confidence_threshold: Some(personal.pii.confidence_threshold),
            column_name_boost: Some(personal.pii.column_name_boost),
            column_denylist: personal.pii.column_denylist.clone(),
            column_allowlist: personal.pii.column_allowlist.clone(),
        },
    };

    let body = serde_yaml::to_string(&project)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to serialize team config: {e}")));
    let header =
        "# Gate team configuration — exported from a personal config, commit this file to git.\n\
        #\n\
        # Every developer's `gate` merges this over their personal\n\
        # ~/.config/gate/config.yaml. tools, pii.patterns, and pii.column_denylist are\n\
        # unioned (added, never removed); pii.confidence_threshold and\n\
        # pii.column_name_boost take whichever value is HIGHER between this file and a\n\
        # developer's personal config.\n\
        #\n\
        # pii.column_allowlist is the one exception: it's unioned too, but it REDUCES\n\
        # redaction (columns here skip name-based checks for everyone). Review the list\n\
        # below before committing — anything here applies to the whole team.\n\
        #\n\
        # Run `gate validate` to see the effective merged config and its provenance.\n\n";
    let contents = format!("{header}{body}");

    write_text_atomic(path, &contents)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to write team config: {e}")));

    println!(
        "Created team config at {} (from personal config)",
        path.display()
    );
    println!(
        "  Exported: {} tools, {} patterns, {} column_denylist entries, {} column_allowlist entries",
        project.tools.len(),
        project.pii.patterns.len(),
        project.pii.column_denylist.len(),
        project.pii.column_allowlist.len(),
    );
    if !project.pii.column_allowlist.is_empty() {
        println!(
            "  Note: column_allowlist entries reduce redaction for the whole team once committed — {}",
            project.pii.column_allowlist.join(", ")
        );
    }
    println!(
        "  Not exported (stays personal): enabled, action, wildcard_policy, redaction format, hash_values/hash_salt, mcp, stats"
    );
    println!("Review the file, then commit it so the team shares it:");
    println!("  git add {}", path.display());
    println!("  git commit -m \"add gate team config\"");
}

fn register_mcp_server(path: &Path, server_name: &str, cmd_str: &str) {
    let upstream_parts = match shell_words::split(cmd_str) {
        Ok(parts) if !parts.is_empty() => parts,
        Ok(_) => exit_with_error("--mcp-cmd must not be empty"),
        Err(e) => exit_with_error(&format!("invalid --mcp-cmd: {e}")),
    };

    // gate mcp --name <server> -- <upstream parts...>
    let mut args: Vec<Value> = vec![
        json!("mcp"),
        json!("--name"),
        json!(server_name),
        json!("--"),
    ];
    args.extend(upstream_parts.iter().map(|s| json!(s)));

    let server_entry = json!({
        "command": "gate",
        "args": args,
        "env": {}
    });

    let mut settings = read_settings(path);
    normalize_mcp_servers(&mut settings);
    settings["mcpServers"][server_name] = server_entry;

    write_atomic(path, &settings)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to write {}: {e}", path.display())));
    println!(
        "MCP server '{}' registered in {} (command: gate mcp -- {})",
        server_name,
        path.display(),
        upstream_parts.join(" ")
    );
    println!("Run `gate mcp -- {cmd_str}` to test the proxy manually.");
}

fn normalize_mcp_servers(settings: &mut Value) {
    if !settings.is_object() {
        *settings = json!({});
    }
    let obj = settings.as_object_mut().unwrap();
    let entry = obj
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
}

/// Resolve the Claude Code hook settings path for the given scope.
/// "project" → ./.claude/settings.json; anything else ("user", "global") → ~/.claude/settings.json.
pub(crate) fn claude_settings_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from(".claude").join("settings.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home).join(".claude").join("settings.json"))
}

/// Resolve the Claude Code MCP config path for the given scope.
/// "project" → ./.mcp.json; anything else ("user", "global") → ~/.claude.json.
fn claude_code_mcp_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from(".mcp.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home).join(".claude.json"))
}

/// Resolve the Copilot CLI MCP config path for the given scope.
/// "project" → ./.mcp.json (same shared format as Claude Code project scope);
/// anything else ("user", "global") → ~/.copilot/mcp-config.json.
fn copilot_mcp_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from(".mcp.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home).join(".copilot").join("mcp-config.json"))
}

/// Resolve the opencode config path for the given scope.
/// "project" → ./opencode.json; anything else ("user", "global") → ~/.config/opencode/opencode.json.
fn opencode_config_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from("opencode.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("opencode")
        .join("opencode.json"))
}

fn register_mcp_server_opencode(path: &Path, server_name: &str, cmd_str: &str) {
    let upstream_parts = match shell_words::split(cmd_str) {
        Ok(parts) if !parts.is_empty() => parts,
        Ok(_) => exit_with_error("--mcp-cmd must not be empty"),
        Err(e) => exit_with_error(&format!("invalid --mcp-cmd: {e}")),
    };

    let mut command: Vec<Value> = vec![
        json!("gate"),
        json!("mcp"),
        json!("--name"),
        json!(server_name),
        json!("--"),
    ];
    command.extend(upstream_parts.iter().map(|s| json!(s)));

    let server_entry = json!({
        "type": "local",
        "command": command,
    });

    let mut config = if path.exists() {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => exit_with_error(&format!("failed to read {}: {e}", path.display())),
        };
        match serde_json::from_str::<Value>(&content) {
            Ok(v) => v,
            Err(e) => exit_with_error(&format!("failed to parse {}: {e}", path.display())),
        }
    } else {
        json!({})
    };

    if !config.get("mcp").is_some_and(|v| v.is_object()) {
        config["mcp"] = json!({});
    }
    config["mcp"][server_name] = server_entry;

    write_atomic(path, &config)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to write {}: {e}", path.display())));
    println!(
        "MCP server '{}' registered in {} (command: gate mcp -- {})",
        server_name,
        path.display(),
        upstream_parts.join(" ")
    );
    println!("Run `gate mcp -- {cmd_str}` to test the proxy manually.");
}

/// Parse a comma-separated `--servers` value into a sorted, deduplicated list.
/// Returns `None` if `raw` is `None` (meaning "wrap all").
fn parse_servers_filter(raw: Option<&str>) -> Option<Vec<String>> {
    raw.map(|s| {
        let mut names: Vec<String> = s
            .split(',')
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect();
        names.sort();
        names.dedup();
        names
    })
}

/// Returns true if an MCP server entry is already proxied through `gate mcp`.
/// Handles both claude-code format { "command": "gate", "args": ["mcp", ...] }
/// and opencode format { "command": ["gate", "mcp", ...] }.
pub(crate) fn is_gate_mcp_proxy(entry: &Value) -> bool {
    let cmd = entry.get("command");
    // claude-code: command is the string "gate", args[0] is "mcp"
    let claude_format = cmd.and_then(|c| c.as_str()) == Some("gate")
        && entry
            .get("args")
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            == Some("mcp");
    // opencode: command is an array ["gate", "mcp", ...]
    let opencode_format = cmd
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.first().and_then(|v| v.as_str()) == Some("gate")
                && arr.get(1).and_then(|v| v.as_str()) == Some("mcp")
        })
        .unwrap_or(false);
    claude_format || opencode_format
}

/// Convert existing MCP servers in a claude-code config (mcpServers key) to gate proxies.
fn wrap_mcp_claude(path: &Path, filter: Option<&[String]>, apply: bool) {
    let settings = read_settings(path);

    let Some(servers) = settings.get("mcpServers").and_then(|v| v.as_object()) else {
        println!("No MCP servers found in {}.", path.display());
        return;
    };

    // (name, upstream_cmd, upstream_args)
    let mut to_wrap: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut already_proxied: Vec<String> = Vec::new();

    for (name, entry) in servers {
        if let Some(f) = filter {
            if !f.contains(name) {
                continue;
            }
        }
        if is_gate_mcp_proxy(entry) {
            already_proxied.push(name.clone());
            continue;
        }
        let Some(cmd) = entry.get("command").and_then(|c| c.as_str()) else {
            continue;
        };
        let args: Vec<String> = entry
            .get("args")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        to_wrap.push((name.clone(), cmd.to_string(), args));
    }

    warn_unknown_servers(filter, servers.keys().map(String::as_str));
    print_wrap_plan(&to_wrap, &already_proxied, path, apply);

    if !apply || to_wrap.is_empty() {
        return;
    }

    let mut updated = settings.clone();
    for (name, cmd, args) in &to_wrap {
        let new_args: Vec<Value> = [json!("mcp"), json!("--name"), json!(name), json!("--")]
            .into_iter()
            .chain(std::iter::once(json!(cmd)))
            .chain(args.iter().map(|s| json!(s)))
            .collect();
        if let Some(entry) = updated["mcpServers"][name.as_str()].as_object_mut() {
            entry.insert("command".to_string(), json!("gate"));
            entry.insert("args".to_string(), Value::Array(new_args));
        }
    }
    write_atomic(path, &updated)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to write {}: {e}", path.display())));
}

/// Convert existing MCP servers in an opencode config (mcp.servers key) to gate proxies.
fn wrap_mcp_opencode(path: &Path, filter: Option<&[String]>, apply: bool) {
    let settings = if path.exists() {
        let content = std::fs::read_to_string(path).unwrap_or_else(|e| {
            exit_with_error(&format!("failed to read {}: {e}", path.display()))
        });
        serde_json::from_str::<Value>(&content).unwrap_or_else(|e| {
            exit_with_error(&format!("failed to parse {}: {e}", path.display()))
        })
    } else {
        json!({})
    };

    let Some(servers) = settings.get("mcp").and_then(|v| v.as_object()) else {
        println!("No MCP servers found in {}.", path.display());
        return;
    };

    let mut to_wrap: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut already_proxied: Vec<String> = Vec::new();

    for (name, entry) in servers {
        if let Some(f) = filter {
            if !f.contains(name) {
                continue;
            }
        }
        if is_gate_mcp_proxy(entry) {
            already_proxied.push(name.clone());
            continue;
        }
        // opencode command format: array where [0] is the executable
        let Some(command_arr) = entry.get("command").and_then(|c| c.as_array()) else {
            continue;
        };
        let Some(cmd) = command_arr.first().and_then(|v| v.as_str()) else {
            continue;
        };
        let args: Vec<String> = command_arr[1..]
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        to_wrap.push((name.clone(), cmd.to_string(), args));
    }

    warn_unknown_servers(filter, servers.keys().map(String::as_str));
    print_wrap_plan(&to_wrap, &already_proxied, path, apply);

    if !apply || to_wrap.is_empty() {
        return;
    }

    let mut updated = settings.clone();
    for (name, cmd, args) in &to_wrap {
        let new_command: Vec<Value> = [
            json!("gate"),
            json!("mcp"),
            json!("--name"),
            json!(name),
            json!("--"),
        ]
        .into_iter()
        .chain(std::iter::once(json!(cmd)))
        .chain(args.iter().map(|s| json!(s)))
        .collect();
        if let Some(entry) = updated["mcp"][name.as_str()].as_object_mut() {
            entry.insert("command".to_string(), Value::Array(new_command));
        }
    }
    write_atomic(path, &updated)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to write {}: {e}", path.display())));
}

/// Warn about any names in `filter` that do not appear in `known`.
fn warn_unknown_servers<'a>(filter: Option<&[String]>, known: impl Iterator<Item = &'a str>) {
    let Some(f) = filter else { return };
    let known_set: std::collections::HashSet<&str> = known.collect();
    let unknown: Vec<&str> = f
        .iter()
        .filter(|n| !known_set.contains(n.as_str()))
        .map(String::as_str)
        .collect();
    for name in unknown {
        eprintln!("warning: server '{name}' not found in config");
    }
}

fn print_wrap_plan(
    to_wrap: &[(String, String, Vec<String>)],
    already_proxied: &[String],
    path: &Path,
    apply: bool,
) {
    if to_wrap.is_empty() {
        if already_proxied.is_empty() {
            println!("No MCP servers found in {}.", path.display());
        } else {
            println!(
                "All MCP servers in {} are already proxied through gate.",
                path.display()
            );
        }
        return;
    }

    let total = to_wrap.len() + already_proxied.len();
    let verb = if apply { "Converted" } else { "Would convert" };
    println!(
        "{} {} of {} MCP server{} in {}:\n",
        verb,
        to_wrap.len(),
        total,
        if total == 1 { "" } else { "s" },
        path.display()
    );

    for (name, cmd, args) in to_wrap {
        let before = if args.is_empty() {
            cmd.clone()
        } else {
            format!("{} {}", cmd, args.join(" "))
        };
        let header = format!("gate mcp --name {name} --");
        let after_parts: Vec<&str> = std::iter::once(header.as_str())
            .chain(std::iter::once(cmd.as_str()))
            .chain(args.iter().map(String::as_str))
            .collect();
        println!("  {}: {} → {}", name, before, after_parts.join(" "));
    }

    if !already_proxied.is_empty() {
        println!(
            "\n  (already proxied, skipped: {})",
            already_proxied.join(", ")
        );
    }

    if !apply {
        println!("\nRun with --yes to apply.");
    }
}

// ── Copilot CLI hook installation ────────────────────────────────────────────

/// Resolve the Copilot CLI command-hook path for the given scope.
/// "project" → <repo_root>/.github/hooks/PreToolUse.json (requires a git repository);
/// anything else ("user", "global") → ~/.copilot/hooks/PreToolUse.json.
pub(crate) fn copilot_hooks_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        let root = find_git_root().ok_or_else(|| {
            "not inside a git repository; gate init --harness copilot-cli --scope project requires a git repository"
                .to_string()
        })?;
        return Ok(root.join(".github").join("hooks").join("PreToolUse.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home)
        .join(".copilot")
        .join("hooks")
        .join("PreToolUse.json"))
}

fn init_copilot_cli(scope: &str) {
    let path = match copilot_hooks_path(scope) {
        Ok(p) => p,
        Err(e) => exit_with_error(&e),
    };
    run_copilot_with_path(&path);
}

fn run_copilot_with_path(path: &Path) {
    let settings = read_copilot_hook_file(path);
    match insert_copilot_hook(settings) {
        CopilotHookResult::AlreadyInstalled => {
            println!("gate hook is already installed in {}", path.display());
        }
        CopilotHookResult::Done(updated) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                    exit_with_error(&format!("failed to create {}: {e}", parent.display()))
                });
            }
            write_atomic(path, &updated).unwrap_or_else(|e| {
                exit_with_error(&format!("failed to write {}: {e}", path.display()))
            });
            println!("gate hook installed in {}", path.display());
            println!("Run `gate config` to define which tools to intercept.");
        }
    }
}

enum CopilotHookResult {
    AlreadyInstalled,
    Done(Value),
}

fn read_copilot_hook_file(path: &Path) -> Value {
    if !path.exists() {
        return json!({"version": 1, "hooks": {}});
    }
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to read {}: {e}", path.display())));
    serde_json::from_str(&contents)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to parse {}: {e}", path.display())))
}

fn insert_copilot_hook(mut settings: Value) -> CopilotHookResult {
    normalize_copilot_settings(&mut settings);

    let arr = settings["hooks"]["PreToolUse"].as_array().unwrap();
    if arr
        .iter()
        .any(|e| e.get("bash").and_then(|b| b.as_str()) == Some(COPILOT_HOOK_COMMAND))
    {
        return CopilotHookResult::AlreadyInstalled;
    }

    let arr = settings["hooks"]["PreToolUse"].as_array_mut().unwrap();
    arr.retain(|entry| !copilot_entry_has_gate_hook(entry));
    arr.push(json!({"type": "command", "bash": COPILOT_HOOK_COMMAND}));

    CopilotHookResult::Done(settings)
}

fn normalize_copilot_settings(settings: &mut Value) {
    if !settings.is_object() {
        *settings = json!({"version": 1, "hooks": {}});
    }
    let obj = settings.as_object_mut().unwrap();
    obj.entry("version".to_string()).or_insert(json!(1));
    let hooks = obj.entry("hooks".to_string()).or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let pretu = hooks
        .as_object_mut()
        .unwrap()
        .entry("PreToolUse".to_string())
        .or_insert_with(|| json!([]));
    if !pretu.is_array() {
        *pretu = json!([]);
    }
}

/// Returns true if the entry's `bash` field is any variant of `gate hook ...`.
pub(crate) fn copilot_entry_has_gate_hook(entry: &Value) -> bool {
    entry
        .get("bash")
        .and_then(|b| b.as_str())
        .map(is_gate_hook_variant)
        .unwrap_or(false)
}

/// Resolve the Cursor MCP config path for the given scope.
/// "project" → `.cursor/mcp.json`; anything else ("user", "global") → `~/.cursor/mcp.json`.
pub(crate) fn cursor_mcp_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from(".cursor").join("mcp.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home).join(".cursor").join("mcp.json"))
}

// ── Codex MCP registration / wrap (TOML) ────────────────────────────────────

/// Resolve the Codex config path for the given scope.
/// "project" → `.codex/config.toml`; anything else ("user", "global") → `~/.codex/config.toml`.
pub(crate) fn codex_config_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from(".codex").join("config.toml"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home).join(".codex").join("config.toml"))
}

fn read_toml_doc(path: &Path) -> toml_edit::DocumentMut {
    if !path.exists() {
        return toml_edit::DocumentMut::new();
    }
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to read {}: {e}", path.display())));
    contents
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_else(|e| exit_with_error(&format!("failed to parse {}: {e}", path.display())))
}

fn write_toml_atomic(path: &Path, doc: &toml_edit::DocumentMut) -> anyhow::Result<()> {
    let toml_str = doc.to_string();
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("config path has no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("config path has no filename"))?;
    let tmp_path = parent.join(format!("{file_name}.gate_tmp"));
    std::fs::write(&tmp_path, &toml_str)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Returns true if a Codex MCP server entry is already proxied through `gate mcp`.
pub(crate) fn is_codex_gate_mcp_proxy(entry: &toml_edit::Item) -> bool {
    entry.get("command").and_then(|c| c.as_str()) == Some("gate")
        && entry
            .get("args")
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.iter().next())
            .and_then(|v| v.as_str())
            == Some("mcp")
}

fn register_mcp_server_codex(path: &Path, server_name: &str, cmd_str: &str) {
    let upstream_parts = match shell_words::split(cmd_str) {
        Ok(parts) if !parts.is_empty() => parts,
        Ok(_) => exit_with_error("--mcp-cmd must not be empty"),
        Err(e) => exit_with_error(&format!("invalid --mcp-cmd: {e}")),
    };

    let mut args_arr = toml_edit::Array::new();
    args_arr.push("mcp");
    args_arr.push("--name");
    args_arr.push(server_name);
    args_arr.push("--");
    for part in &upstream_parts {
        args_arr.push(part.as_str());
    }

    let mut doc = read_toml_doc(path);

    if !doc.contains_key("mcp_servers") {
        doc.insert(
            "mcp_servers",
            toml_edit::Item::Table(toml_edit::Table::new()),
        );
    }

    let mcp_servers = doc["mcp_servers"]
        .as_table_mut()
        .unwrap_or_else(|| exit_with_error("mcp_servers in config.toml is not a table"));

    let mut server_table = toml_edit::Table::new();
    server_table["command"] = toml_edit::value("gate");
    server_table["args"] = toml_edit::value(args_arr);
    mcp_servers.insert(server_name, toml_edit::Item::Table(server_table));

    write_toml_atomic(path, &doc)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to write {}: {e}", path.display())));
    println!(
        "MCP server '{}' registered in {} (command: gate mcp -- {})",
        server_name,
        path.display(),
        upstream_parts.join(" ")
    );
    println!("Run `gate mcp -- {cmd_str}` to test the proxy manually.");
}

fn wrap_mcp_codex(path: &Path, filter: Option<&[String]>, apply: bool) {
    let mut doc = read_toml_doc(path);

    let (to_wrap, already_proxied, server_names) = {
        let Some(mcp_servers) = doc.get("mcp_servers").and_then(|s| s.as_table()) else {
            println!("No MCP servers found in {}.", path.display());
            return;
        };

        let mut to_wrap: Vec<(String, String, Vec<String>)> = Vec::new();
        let mut already_proxied: Vec<String> = Vec::new();
        let server_names: Vec<String> = mcp_servers.iter().map(|(k, _)| k.to_string()).collect();

        for (name, entry) in mcp_servers.iter() {
            if let Some(f) = filter {
                if !f.iter().any(|n| n == name) {
                    continue;
                }
            }

            if is_codex_gate_mcp_proxy(entry) {
                already_proxied.push(name.to_string());
                continue;
            }

            let Some(cmd) = entry.get("command").and_then(|c| c.as_str()) else {
                eprintln!(
                    "warning: server '{name}' has no stdio command — skipping \
                     (HTTP servers cannot be proxied by gate mcp)"
                );
                continue;
            };

            let args: Vec<String> = entry
                .get("args")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            to_wrap.push((name.to_string(), cmd.to_string(), args));
        }

        (to_wrap, already_proxied, server_names)
    };

    warn_unknown_servers(filter, server_names.iter().map(String::as_str));
    print_wrap_plan(&to_wrap, &already_proxied, path, apply);

    if !apply || to_wrap.is_empty() {
        return;
    }

    for (name, cmd, args_vec) in &to_wrap {
        let mut new_args = toml_edit::Array::new();
        new_args.push("mcp");
        new_args.push("--name");
        new_args.push(name.as_str());
        new_args.push("--");
        new_args.push(cmd.as_str());
        for arg in args_vec {
            new_args.push(arg.as_str());
        }
        doc["mcp_servers"][name.as_str()]["command"] = toml_edit::value("gate");
        doc["mcp_servers"][name.as_str()]["args"] = toml_edit::value(new_args);
    }

    write_toml_atomic(path, &doc)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to write {}: {e}", path.display())));
}

// ── Cursor hook installation ─────────────────────────────────────────────────

/// Resolve the Cursor hooks config path for the given scope.
/// "project" → `.cursor/hooks.json`; anything else ("user", "global") → `~/.cursor/hooks.json`.
pub(crate) fn cursor_hooks_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from(".cursor").join("hooks.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home).join(".cursor").join("hooks.json"))
}

fn init_cursor(scope: &str) {
    let path = match cursor_hooks_path(scope) {
        Ok(p) => p,
        Err(e) => exit_with_error(&format!("cannot resolve cursor hooks path: {e}")),
    };
    run_cursor_with_path(&path);
}

fn run_cursor_with_path(path: &Path) {
    let settings = read_cursor_hook_file(path);
    match insert_cursor_hook(settings) {
        CursorHookResult::AlreadyInstalled => {
            println!("gate hook is already installed in {}", path.display());
        }
        CursorHookResult::Done(updated) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                    exit_with_error(&format!("failed to create {}: {e}", parent.display()))
                });
            }
            write_atomic(path, &updated).unwrap_or_else(|e| {
                exit_with_error(&format!("failed to write {}: {e}", path.display()))
            });
            println!("gate hook installed in {}", path.display());
            println!("Run `gate config` to define which tools to intercept.");
        }
    }
}

enum CursorHookResult {
    AlreadyInstalled,
    Done(Value),
}

fn read_cursor_hook_file(path: &Path) -> Value {
    if !path.exists() {
        return json!({"version": 1, "hooks": {}});
    }
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to read {}: {e}", path.display())));
    serde_json::from_str(&contents)
        .unwrap_or_else(|e| exit_with_error(&format!("failed to parse {}: {e}", path.display())))
}

fn insert_cursor_hook(mut settings: Value) -> CursorHookResult {
    normalize_cursor_settings(&mut settings);

    let arr = settings["hooks"]["preToolUse"].as_array().unwrap();
    if arr
        .iter()
        .any(|e| e.get("command").and_then(|c| c.as_str()) == Some(CURSOR_HOOK_COMMAND))
    {
        return CursorHookResult::AlreadyInstalled;
    }

    let arr = settings["hooks"]["preToolUse"].as_array_mut().unwrap();
    arr.retain(|entry| !cursor_entry_has_gate_hook(entry));
    arr.push(json!({"command": CURSOR_HOOK_COMMAND}));

    CursorHookResult::Done(settings)
}

fn normalize_cursor_settings(settings: &mut Value) {
    if !settings.is_object() {
        *settings = json!({"version": 1, "hooks": {}});
    }
    let obj = settings.as_object_mut().unwrap();
    obj.entry("version".to_string()).or_insert(json!(1));
    let hooks = obj.entry("hooks".to_string()).or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let pretu = hooks
        .as_object_mut()
        .unwrap()
        .entry("preToolUse".to_string())
        .or_insert_with(|| json!([]));
    if !pretu.is_array() {
        *pretu = json!([]);
    }
}

/// Returns true if the entry's `command` field is any variant of `gate hook ...`.
pub(crate) fn cursor_entry_has_gate_hook(entry: &Value) -> bool {
    entry
        .get("command")
        .and_then(|c| c.as_str())
        .map(is_gate_hook_variant)
        .unwrap_or(false)
}

// ── Codex hook installation ──────────────────────────────────────────────────

/// Resolve the Codex hooks config path for the given scope.
/// "project" → `.codex/hooks.json`; anything else ("user", "global") → `~/.codex/hooks.json`.
pub(crate) fn codex_hooks_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from(".codex").join("hooks.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home).join(".codex").join("hooks.json"))
}

fn init_codex(scope: &str) {
    let path = match codex_hooks_path(scope) {
        Ok(p) => p,
        Err(e) => exit_with_error(&format!("cannot resolve codex hooks path: {e}")),
    };
    run_codex_with_path(&path, scope);
}

fn run_codex_with_path(path: &Path, scope: &str) {
    let settings = read_settings(path);
    match insert_codex_hook(settings) {
        CodexHookResult::AlreadyInstalled => {
            println!("gate hook is already installed in {}", path.display());
        }
        CodexHookResult::Done(updated) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                    exit_with_error(&format!("failed to create {}: {e}", parent.display()))
                });
            }
            write_atomic(path, &updated).unwrap_or_else(|e| {
                exit_with_error(&format!("failed to write {}: {e}", path.display()))
            });
            println!("gate hook installed in {}", path.display());
            println!("Run `gate config` to define which tools to intercept.");
            if scope == "project" {
                println!("Note: project-scope hooks only take effect after you trust this repo in Codex.");
            }
        }
    }
}

enum CodexHookResult {
    AlreadyInstalled,
    Done(Value),
}

fn new_codex_hook_entry() -> Value {
    json!({
        "matcher": "^Bash$",
        "hooks": [{ "type": "command", "command": CODEX_HOOK_COMMAND }]
    })
}

fn insert_codex_hook(mut settings: Value) -> CodexHookResult {
    normalize_settings(&mut settings);

    let already = {
        let arr = settings["hooks"]["PreToolUse"].as_array().unwrap();
        arr.iter().any(|entry| {
            entry
                .get("hooks")
                .and_then(|h| h.as_array())
                .map(|hooks| {
                    hooks.iter().any(|h| {
                        h.get("command").and_then(|c| c.as_str()) == Some(CODEX_HOOK_COMMAND)
                    })
                })
                .unwrap_or(false)
        })
    };
    if already {
        return CodexHookResult::AlreadyInstalled;
    }

    {
        let arr = settings["hooks"]["PreToolUse"].as_array_mut().unwrap();
        arr.retain(|entry| !entry_has_gate_hook(entry));
        arr.push(new_codex_hook_entry());
    }

    CodexHookResult::Done(settings)
}

// ── Gemini CLI hook installation ─────────────────────────────────────────────

/// Resolve the Gemini CLI settings path for the given scope.
/// Hooks and MCP servers share the same file.
/// "project" → `.gemini/settings.json`; anything else → `~/.gemini/settings.json`.
pub(crate) fn gemini_settings_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from(".gemini").join("settings.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home).join(".gemini").join("settings.json"))
}

/// Returns true if a `hooks.BeforeTool` entry contains any variant of `gate hook ...`.
pub(crate) fn gemini_entry_has_gate_hook(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(is_gate_hook_variant)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn new_gemini_hook_entry() -> Value {
    json!({
        "matcher": "^run_shell_command$",
        "hooks": [{ "type": "command", "command": GEMINI_HOOK_COMMAND }]
    })
}

fn init_gemini(scope: &str) {
    let path = match gemini_settings_path(scope) {
        Ok(p) => p,
        Err(e) => exit_with_error(&format!("cannot resolve gemini settings path: {e}")),
    };
    run_gemini_with_path(&path);
}

fn run_gemini_with_path(path: &Path) {
    let settings = read_settings(path);
    match insert_gemini_hook(settings) {
        GeminiHookResult::AlreadyInstalled => {
            println!("gate hook is already installed in {}", path.display());
        }
        GeminiHookResult::Done(updated) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                    exit_with_error(&format!("failed to create {}: {e}", parent.display()))
                });
            }
            write_atomic(path, &updated).unwrap_or_else(|e| {
                exit_with_error(&format!("failed to write {}: {e}", path.display()))
            });
            println!("gate hook installed in {}", path.display());
            println!("Run `gate config` to define which tools to intercept.");
        }
    }
}

enum GeminiHookResult {
    AlreadyInstalled,
    Done(Value),
}

fn insert_gemini_hook(mut settings: Value) -> GeminiHookResult {
    normalize_gemini_settings(&mut settings);

    let already = {
        let arr = settings["hooks"]["BeforeTool"].as_array().unwrap();
        arr.iter().any(|entry| {
            entry
                .get("hooks")
                .and_then(|h| h.as_array())
                .map(|hooks| {
                    hooks.iter().any(|h| {
                        h.get("command").and_then(|c| c.as_str()) == Some(GEMINI_HOOK_COMMAND)
                    })
                })
                .unwrap_or(false)
        })
    };
    if already {
        return GeminiHookResult::AlreadyInstalled;
    }

    {
        let arr = settings["hooks"]["BeforeTool"].as_array_mut().unwrap();
        arr.retain(|entry| !gemini_entry_has_gate_hook(entry));
        arr.push(new_gemini_hook_entry());
    }

    GeminiHookResult::Done(settings)
}

fn normalize_gemini_settings(settings: &mut Value) {
    if !settings.is_object() {
        *settings = json!({});
    }
    let obj = settings.as_object_mut().unwrap();

    let hooks = obj.entry("hooks".to_string()).or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }

    let before_tool = hooks
        .as_object_mut()
        .unwrap()
        .entry("BeforeTool".to_string())
        .or_insert_with(|| json!([]));
    if !before_tool.is_array() {
        *before_tool = json!([]);
    }
}

// ── CodeBuddy hook installation ──────────────────────────────────────────────

/// Resolve the CodeBuddy settings path for the given scope.
/// "project" → `.codebuddy/settings.json`; anything else ("user", "global") → `~/.codebuddy/settings.json`.
pub(crate) fn codebuddy_settings_path(scope: &str) -> Result<PathBuf, String> {
    if scope == "project" {
        return Ok(PathBuf::from(".codebuddy").join("settings.json"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot resolve home directory: set HOME or USERPROFILE".to_string())?;
    Ok(PathBuf::from(home).join(".codebuddy").join("settings.json"))
}

fn init_codebuddy(scope: &str) {
    let path = match codebuddy_settings_path(scope) {
        Ok(p) => p,
        Err(e) => exit_with_error(&format!("cannot resolve codebuddy settings path: {e}")),
    };
    run_codebuddy_with_path(&path);
}

fn run_codebuddy_with_path(path: &Path) {
    let settings = read_settings(path);
    match insert_codebuddy_hook(settings) {
        HookInsertResult::AlreadyInstalled => {
            println!("gate hook is already installed in {}", path.display());
        }
        HookInsertResult::Done(updated) => {
            write_atomic(path, &updated).unwrap_or_else(|e| {
                exit_with_error(&format!("failed to write {}: {e}", path.display()))
            });
            println!("gate hook installed in {}", path.display());
            println!("Run `gate config` to define which tools to intercept.");
        }
    }
}

fn new_codebuddy_hook_entry() -> Value {
    json!({
        "matcher": "Bash",
        "hooks": [{ "type": "command", "command": CODEBUDDY_HOOK_COMMAND }]
    })
}

fn insert_codebuddy_hook(mut settings: Value) -> HookInsertResult {
    normalize_settings(&mut settings);

    let already = {
        let arr = settings["hooks"]["PreToolUse"].as_array().unwrap();
        arr.iter().any(|entry| {
            entry
                .get("hooks")
                .and_then(|h| h.as_array())
                .map(|hooks| {
                    hooks.iter().any(|h| {
                        h.get("command").and_then(|c| c.as_str()) == Some(CODEBUDDY_HOOK_COMMAND)
                    })
                })
                .unwrap_or(false)
        })
    };
    if already {
        return HookInsertResult::AlreadyInstalled;
    }

    {
        let arr = settings["hooks"]["PreToolUse"].as_array_mut().unwrap();
        arr.retain(|entry| !entry_has_gate_hook(entry));
        arr.push(new_codebuddy_hook_entry());
    }

    HookInsertResult::Done(settings)
}

/// Walk up from CWD to find the root of the current git repository.
pub(crate) fn find_git_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    static HOME_LOCK: Mutex<()> = Mutex::new(());
    static GATE_CONFIG_LOCK: Mutex<()> = Mutex::new(());

    fn tmp_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        (dir, path)
    }

    // insert_hook unit tests

    #[test]
    fn insert_into_empty_object() {
        let settings = json!({});
        let result = insert_hook(settings);
        assert!(matches!(result, HookInsertResult::Done(_)));
        if let HookInsertResult::Done(v) = result {
            assert_eq!(
                v["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
                HOOK_COMMAND
            );
        }
    }

    #[test]
    fn already_installed_returns_already() {
        let settings = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "gate hook" }] }
                ]
            }
        });
        assert!(matches!(
            insert_hook(settings),
            HookInsertResult::AlreadyInstalled
        ));
    }

    #[test]
    fn replaces_absolute_path_variant() {
        let settings = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "/usr/local/bin/gate hook" }] }
                ]
            }
        });
        if let HookInsertResult::Done(v) = insert_hook(settings) {
            let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0]["hooks"][0]["command"], HOOK_COMMAND);
        } else {
            panic!("expected Done");
        }
    }

    #[test]
    fn preserves_unrelated_entries() {
        let settings = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "some-other-hook" }] }
                ]
            }
        });
        if let HookInsertResult::Done(v) = insert_hook(settings) {
            let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
            assert_eq!(arr.len(), 2);
            let commands: Vec<&str> = arr
                .iter()
                .filter_map(|e| e["hooks"][0]["command"].as_str())
                .collect();
            assert!(commands.contains(&"some-other-hook"));
            assert!(commands.contains(&HOOK_COMMAND));
        } else {
            panic!("expected Done");
        }
    }

    #[test]
    fn non_array_pretu_is_replaced() {
        let settings = json!({ "hooks": { "PreToolUse": "unexpected_string" } });
        if let HookInsertResult::Done(v) = insert_hook(settings) {
            assert!(v["hooks"]["PreToolUse"].is_array());
        } else {
            panic!("expected Done");
        }
    }

    // run_with_path integration tests

    #[test]
    fn creates_settings_when_file_missing() {
        let (_dir, path) = tmp_path();
        run_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            v["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            HOOK_COMMAND
        );
    }

    #[test]
    fn idempotent_on_second_run() {
        let (_dir, path) = tmp_path();
        run_with_path(&path);
        run_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        let gate_count = arr.iter().filter(|e| entry_has_gate_hook(e)).count();
        assert_eq!(gate_count, 1);
    }

    #[test]
    fn replaces_variant_on_disk() {
        let (_dir, path) = tmp_path();
        let initial = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "/usr/local/bin/gate hook" }] }
                ]
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["hooks"][0]["command"], HOOK_COMMAND);
    }

    #[test]
    fn creates_parent_dir_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/subdir/settings.json");
        run_with_path(&path);
        assert!(path.exists());
    }

    #[test]
    fn write_is_valid_json() {
        let (_dir, path) = tmp_path();
        run_with_path(&path);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(serde_json::from_str::<Value>(&contents).is_ok());
    }

    // claude_settings_path

    #[test]
    fn claude_settings_path_global_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = claude_settings_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.claude/settings.json"));
    }

    #[test]
    fn claude_settings_path_project_is_relative() {
        let path = claude_settings_path("project").unwrap();
        assert_eq!(path, PathBuf::from(".claude/settings.json"));
    }

    // claude_code_mcp_path

    #[test]
    fn mcp_path_default_scope_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = claude_code_mcp_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.claude.json"));
    }

    #[test]
    fn mcp_path_user_scope_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = claude_code_mcp_path("user").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.claude.json"));
    }

    #[test]
    fn mcp_path_project_scope_is_relative() {
        let path = claude_code_mcp_path("project").unwrap();
        assert_eq!(path, PathBuf::from(".mcp.json"));
    }

    // opencode_config_path

    #[test]
    fn opencode_config_path_global_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = opencode_config_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(
            path,
            PathBuf::from("/test/home/.config/opencode/opencode.json")
        );
    }

    #[test]
    fn opencode_config_path_user_uses_home() {
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = opencode_config_path("user").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(
            path,
            PathBuf::from("/test/home/.config/opencode/opencode.json")
        );
    }

    #[test]
    fn opencode_config_path_project_is_relative() {
        let path = opencode_config_path("project").unwrap();
        assert_eq!(path, PathBuf::from("opencode.json"));
    }

    // copilot_mcp_path

    #[test]
    fn copilot_mcp_path_global_uses_home() {
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = copilot_mcp_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.copilot/mcp-config.json"));
    }

    #[test]
    fn copilot_mcp_path_user_uses_home() {
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = copilot_mcp_path("user").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.copilot/mcp-config.json"));
    }

    #[test]
    fn copilot_mcp_path_project_is_relative() {
        let path = copilot_mcp_path("project").unwrap();
        assert_eq!(path, PathBuf::from(".mcp.json"));
    }

    // copilot_hooks_path

    #[test]
    fn copilot_hooks_path_global_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = copilot_hooks_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(
            path,
            PathBuf::from("/test/home/.copilot/hooks/PreToolUse.json")
        );
    }

    #[test]
    fn copilot_hooks_path_user_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = copilot_hooks_path("user").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(
            path,
            PathBuf::from("/test/home/.copilot/hooks/PreToolUse.json")
        );
    }

    #[test]
    fn copilot_hooks_path_project_requires_git_repo() {
        let dir = tempfile::tempdir().unwrap();
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = copilot_hooks_path("project");
        std::env::set_current_dir(saved).unwrap();
        assert!(result.is_err());
    }

    // register_mcp_server (claude-code, project scope → .mcp.json)

    #[test]
    fn mcp_server_project_scope_written_to_mcp_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");
        register_mcp_server(&path, "postgres", "uvx mcp-server-postgres");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["postgres"]["command"], "gate");
        let args = v["mcpServers"]["postgres"]["args"].as_array().unwrap();
        assert_eq!(args[0], "mcp");
        assert_eq!(args[1], "--name");
        assert_eq!(args[2], "postgres");
        assert_eq!(args[3], "--");
        assert_eq!(args[4], "uvx");
    }

    #[test]
    fn mcp_server_project_scope_preserves_existing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");
        let initial = json!({"mcpServers": {"other": {"command": "other", "args": []}}});
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        register_mcp_server(&path, "postgres", "uvx mcp-server-postgres");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(v["mcpServers"]["other"].is_object());
        assert!(v["mcpServers"]["postgres"].is_object());
    }

    // register_mcp_server (claude-code, user scope → ~/.claude.json)

    #[test]
    fn mcp_server_written_to_empty_settings() {
        let (_dir, path) = tmp_path();
        register_mcp_server(&path, "postgres", "uvx mcp-server-postgres");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["postgres"]["command"], "gate");
        let args = v["mcpServers"]["postgres"]["args"].as_array().unwrap();
        assert_eq!(args[0], "mcp");
        assert_eq!(args[1], "--name");
        assert_eq!(args[2], "postgres");
        assert_eq!(args[3], "--");
        assert_eq!(args[4], "uvx");
    }

    #[test]
    fn mcp_server_preserves_existing_entries() {
        let (_dir, path) = tmp_path();
        let initial = json!({"mcpServers": {"other": {"command": "other", "args": []}}});
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        register_mcp_server(&path, "postgres", "uvx mcp-server-postgres");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(v["mcpServers"]["other"].is_object());
        assert!(v["mcpServers"]["postgres"].is_object());
    }

    #[test]
    fn mcp_server_overwrites_existing_entry_with_same_name() {
        let (_dir, path) = tmp_path();
        register_mcp_server(&path, "postgres", "uvx mcp-server-postgres --old");
        register_mcp_server(&path, "postgres", "uvx mcp-server-postgres --new");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let args = v["mcpServers"]["postgres"]["args"].as_array().unwrap();
        assert!(args.iter().any(|a| a.as_str() == Some("--new")));
    }

    // register_mcp_server_opencode

    #[test]
    fn opencode_mcp_server_written_to_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.json");
        register_mcp_server_opencode(&path, "postgres", "uvx mcp-server-postgres");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcp"]["postgres"]["type"], "local");
        let cmd = v["mcp"]["postgres"]["command"].as_array().unwrap();
        assert_eq!(cmd[0], "gate");
        assert_eq!(cmd[1], "mcp");
        assert_eq!(cmd[2], "--name");
        assert_eq!(cmd[3], "postgres");
        assert_eq!(cmd[4], "--");
        assert_eq!(cmd[5], "uvx");
    }

    #[test]
    fn opencode_mcp_server_merges_with_existing_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.json");
        let initial =
            json!({"theme": "dark", "mcp": {"github": {"type": "local", "command": ["gh"]}}});
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        register_mcp_server_opencode(&path, "postgres", "uvx mcp-server-postgres");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        assert!(v["mcp"]["github"].is_object());
        assert!(v["mcp"]["postgres"].is_object());
    }

    #[test]
    fn opencode_mcp_server_multi_word_cmd_split_into_args() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.json");
        register_mcp_server_opencode(&path, "pg", "uvx mcp-server-postgres --db mydb");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let cmd = v["mcp"]["pg"]["command"].as_array().unwrap();
        // gate, mcp, --name, pg, --, uvx, mcp-server-postgres, --db, mydb
        assert_eq!(cmd.len(), 9);
        assert_eq!(cmd[2], "--name");
        assert_eq!(cmd[3], "pg");
        assert_eq!(cmd[7], "--db");
        assert_eq!(cmd[8], "mydb");
    }

    // is_gate_hook_variant

    #[test]
    fn variant_matches_exact_command() {
        assert!(is_gate_hook_variant("gate hook"));
    }

    #[test]
    fn variant_matches_absolute_path() {
        assert!(is_gate_hook_variant("/usr/local/bin/gate hook"));
    }

    #[test]
    fn variant_does_not_match_other_commands() {
        assert!(!is_gate_hook_variant("gate run -- tkpsql"));
        assert!(!is_gate_hook_variant("some-tool run"));
        assert!(!is_gate_hook_variant(""));
    }

    // is_gate_mcp_proxy

    #[test]
    fn proxy_detected_when_command_is_gate_and_first_arg_is_mcp() {
        let entry = json!({"command": "gate", "args": ["mcp", "--", "uvx", "mcp-server-postgres"]});
        assert!(is_gate_mcp_proxy(&entry));
    }

    #[test]
    fn proxy_not_detected_for_non_gate_command() {
        let entry = json!({"command": "uvx", "args": ["mcp-server-postgres"]});
        assert!(!is_gate_mcp_proxy(&entry));
    }

    #[test]
    fn proxy_not_detected_when_gate_but_no_mcp_arg() {
        let entry = json!({"command": "gate", "args": ["run", "--", "uvx"]});
        assert!(!is_gate_mcp_proxy(&entry));
    }

    // wrap_mcp_claude — dry-run

    #[test]
    fn wrap_mcp_claude_dry_run_does_not_modify_file() {
        let (_dir, path) = tmp_path();
        let initial = json!({
            "mcpServers": {
                "postgres": {"command": "uvx", "args": ["mcp-server-postgres"], "env": {}}
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        wrap_mcp_claude(&path, None, false);
        let on_disk: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk["mcpServers"]["postgres"]["command"], "uvx");
    }

    #[test]
    fn wrap_mcp_claude_apply_rewrites_command_and_args() {
        let (_dir, path) = tmp_path();
        let initial = json!({
            "mcpServers": {
                "postgres": {"command": "uvx", "args": ["mcp-server-postgres", "--db", "mydb"], "env": {}}
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        wrap_mcp_claude(&path, None, true);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["postgres"]["command"], "gate");
        let args = v["mcpServers"]["postgres"]["args"].as_array().unwrap();
        assert_eq!(args[0], "mcp");
        assert_eq!(args[1], "--name");
        assert_eq!(args[2], "postgres");
        assert_eq!(args[3], "--");
        assert_eq!(args[4], "uvx");
        assert_eq!(args[5], "mcp-server-postgres");
        assert_eq!(args[6], "--db");
        assert_eq!(args[7], "mydb");
    }

    #[test]
    fn wrap_mcp_claude_apply_preserves_other_fields() {
        let (_dir, path) = tmp_path();
        let initial = json!({
            "mcpServers": {
                "pg": {"command": "uvx", "args": ["mcp-server-postgres"], "env": {"DB": "prod"}}
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        wrap_mcp_claude(&path, None, true);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["pg"]["env"]["DB"], "prod");
    }

    #[test]
    fn wrap_mcp_claude_apply_skips_already_proxied() {
        let (_dir, path) = tmp_path();
        let initial = json!({
            "mcpServers": {
                "already": {"command": "gate", "args": ["mcp", "--", "uvx", "mcp-server-x"]},
                "new":     {"command": "uvx", "args": ["mcp-server-y"]}
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        wrap_mcp_claude(&path, None, true);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // already-proxied entry unchanged
        assert_eq!(v["mcpServers"]["already"]["args"][2], "uvx");
        // new entry converted
        assert_eq!(v["mcpServers"]["new"]["command"], "gate");
    }

    #[test]
    fn wrap_mcp_claude_apply_no_op_when_all_proxied() {
        let (_dir, path) = tmp_path();
        let initial = json!({
            "mcpServers": {
                "pg": {"command": "gate", "args": ["mcp", "--", "uvx", "mcp-server-postgres"]}
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        wrap_mcp_claude(&path, None, true);
        // file must be untouched (no write_atomic called)
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    // wrap_mcp_opencode — apply

    #[test]
    fn wrap_mcp_opencode_apply_rewrites_servers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.json");
        let initial = json!({
            "theme": "dark",
            "mcp": {
                "github": {"type": "local", "command": ["npx", "@mcp/github"]},
                "proxied": {"type": "local", "command": ["gate", "mcp", "--", "uvx", "mcp-server-x"]}
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        wrap_mcp_opencode(&path, None, true);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        let cmd = v["mcp"]["github"]["command"].as_array().unwrap();
        assert_eq!(cmd[0], "gate");
        assert_eq!(cmd[1], "mcp");
        assert_eq!(cmd[2], "--name");
        assert_eq!(cmd[3], "github");
        assert_eq!(cmd[4], "--");
        assert_eq!(cmd[5], "npx");
        // already-proxied entry unchanged
        let proxied_cmd = v["mcp"]["proxied"]["command"].as_array().unwrap();
        assert_eq!(proxied_cmd[3], "uvx");
    }

    // parse_servers_filter

    #[test]
    fn parse_servers_filter_none_returns_none() {
        assert!(parse_servers_filter(None).is_none());
    }

    #[test]
    fn parse_servers_filter_splits_and_trims() {
        let f = parse_servers_filter(Some("postgres, github , stripe")).unwrap();
        assert_eq!(f, vec!["github", "postgres", "stripe"]); // sorted
    }

    #[test]
    fn parse_servers_filter_deduplicates() {
        let f = parse_servers_filter(Some("postgres,postgres")).unwrap();
        assert_eq!(f, vec!["postgres"]);
    }

    #[test]
    fn parse_servers_filter_ignores_empty_segments() {
        let f = parse_servers_filter(Some(",postgres,,")).unwrap();
        assert_eq!(f, vec!["postgres"]);
    }

    // --servers filter applied to wrap_mcp_claude

    #[test]
    fn wrap_mcp_claude_filter_only_wraps_named_servers() {
        let (_dir, path) = tmp_path();
        let initial = json!({
            "mcpServers": {
                "postgres": {"command": "uvx", "args": ["mcp-server-postgres"]},
                "github":   {"command": "npx", "args": ["@mcp/github"]},
                "stripe":   {"command": "npx", "args": ["@mcp/stripe"]}
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        let filter = parse_servers_filter(Some("postgres,github"));
        wrap_mcp_claude(&path, filter.as_deref(), true);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["postgres"]["command"], "gate");
        assert_eq!(v["mcpServers"]["github"]["command"], "gate");
        // stripe excluded from filter — must remain unchanged
        assert_eq!(v["mcpServers"]["stripe"]["command"], "npx");
    }

    #[test]
    fn wrap_mcp_claude_filter_dry_run_leaves_file_unchanged() {
        let (_dir, path) = tmp_path();
        let initial = json!({
            "mcpServers": {
                "postgres": {"command": "uvx", "args": ["mcp-server-postgres"]},
                "github":   {"command": "npx", "args": ["@mcp/github"]}
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let filter = parse_servers_filter(Some("postgres"));
        wrap_mcp_claude(&path, filter.as_deref(), false);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    // ── copilot-cli init ──────────────────────────────────────────────────────

    fn tmp_copilot_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".github/hooks/PreToolUse.json");
        (dir, path)
    }

    #[test]
    fn copilot_creates_hook_file_from_scratch() {
        let (_dir, path) = tmp_copilot_path();
        run_copilot_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["version"], json!(1));
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["bash"].as_str().unwrap(), COPILOT_HOOK_COMMAND);
        assert_eq!(arr[0]["type"].as_str().unwrap(), "command");
    }

    #[test]
    fn copilot_idempotent_on_second_run() {
        let (_dir, path) = tmp_copilot_path();
        run_copilot_with_path(&path);
        run_copilot_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        let gate_count = arr
            .iter()
            .filter(|e| e["bash"].as_str() == Some(COPILOT_HOOK_COMMAND))
            .count();
        assert_eq!(gate_count, 1);
    }

    #[test]
    fn copilot_preserves_existing_hooks() {
        let (_dir, path) = tmp_copilot_path();
        let initial = json!({
            "version": 1,
            "hooks": {
                "PreToolUse": [
                    {"type": "command", "bash": "other-hook --check"}
                ]
            }
        });
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_copilot_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let cmds: Vec<&str> = arr.iter().filter_map(|e| e["bash"].as_str()).collect();
        assert!(cmds.contains(&"other-hook --check"));
        assert!(cmds.contains(&COPILOT_HOOK_COMMAND));
    }

    #[test]
    fn copilot_replaces_old_gate_variant() {
        let (_dir, path) = tmp_copilot_path();
        let initial = json!({
            "version": 1,
            "hooks": {
                "PreToolUse": [
                    {"type": "command", "bash": "/usr/local/bin/gate hook --format copilot"}
                ]
            }
        });
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_copilot_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["bash"].as_str().unwrap(), COPILOT_HOOK_COMMAND);
    }

    #[test]
    fn copilot_write_is_valid_json() {
        let (_dir, path) = tmp_copilot_path();
        run_copilot_with_path(&path);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(serde_json::from_str::<Value>(&contents).is_ok());
    }

    #[test]
    fn copilot_entry_has_gate_hook_detects_exact() {
        let entry = json!({"type": "command", "bash": "gate hook --format copilot"});
        assert!(copilot_entry_has_gate_hook(&entry));
    }

    #[test]
    fn copilot_entry_has_gate_hook_detects_absolute_path() {
        let entry = json!({"type": "command", "bash": "/usr/local/bin/gate hook --format copilot"});
        assert!(copilot_entry_has_gate_hook(&entry));
    }

    #[test]
    fn copilot_entry_has_gate_hook_ignores_other_tools() {
        let entry = json!({"type": "command", "bash": "other-hook --check"});
        assert!(!copilot_entry_has_gate_hook(&entry));
    }

    #[test]
    fn copilot_entry_has_gate_hook_ignores_non_bash_field() {
        let entry = json!({"type": "command", "command": "gate hook --format copilot"});
        assert!(!copilot_entry_has_gate_hook(&entry));
    }

    // ── cursor init ───────────────────────────────────────────────────────────

    fn tmp_cursor_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".cursor/hooks.json");
        (dir, path)
    }

    #[test]
    fn cursor_creates_hook_file_from_scratch() {
        let (_dir, path) = tmp_cursor_path();
        run_cursor_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["version"], json!(1));
        let arr = v["hooks"]["preToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["command"].as_str().unwrap(), CURSOR_HOOK_COMMAND);
    }

    #[test]
    fn cursor_idempotent_on_second_run() {
        let (_dir, path) = tmp_cursor_path();
        run_cursor_with_path(&path);
        run_cursor_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["preToolUse"].as_array().unwrap();
        let gate_count = arr
            .iter()
            .filter(|e| e["command"].as_str() == Some(CURSOR_HOOK_COMMAND))
            .count();
        assert_eq!(gate_count, 1);
    }

    #[test]
    fn cursor_preserves_existing_hooks() {
        let (_dir, path) = tmp_cursor_path();
        let initial = json!({
            "version": 1,
            "hooks": {
                "preToolUse": [
                    {"command": "other-hook --check"}
                ]
            }
        });
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_cursor_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["preToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let cmds: Vec<&str> = arr.iter().filter_map(|e| e["command"].as_str()).collect();
        assert!(cmds.contains(&"other-hook --check"));
        assert!(cmds.contains(&CURSOR_HOOK_COMMAND));
    }

    #[test]
    fn cursor_replaces_absolute_path_variant() {
        let (_dir, path) = tmp_cursor_path();
        let initial = json!({
            "version": 1,
            "hooks": {
                "preToolUse": [
                    {"command": "/usr/local/bin/gate hook --format cursor"}
                ]
            }
        });
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_cursor_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["preToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["command"].as_str().unwrap(), CURSOR_HOOK_COMMAND);
    }

    #[test]
    fn cursor_write_is_valid_json() {
        let (_dir, path) = tmp_cursor_path();
        run_cursor_with_path(&path);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(serde_json::from_str::<Value>(&contents).is_ok());
    }

    #[test]
    fn cursor_entry_has_gate_hook_detects_exact() {
        let entry = json!({"command": "gate hook --format cursor"});
        assert!(cursor_entry_has_gate_hook(&entry));
    }

    #[test]
    fn cursor_entry_has_gate_hook_detects_absolute_path() {
        let entry = json!({"command": "/usr/local/bin/gate hook --format cursor"});
        assert!(cursor_entry_has_gate_hook(&entry));
    }

    #[test]
    fn cursor_entry_has_gate_hook_ignores_other_tools() {
        let entry = json!({"command": "other-hook --check"});
        assert!(!cursor_entry_has_gate_hook(&entry));
    }

    #[test]
    fn cursor_mcp_path_global_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = cursor_mcp_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.cursor/mcp.json"));
    }

    #[test]
    fn cursor_mcp_path_project_is_relative() {
        let path = cursor_mcp_path("project").unwrap();
        assert_eq!(path, PathBuf::from(".cursor/mcp.json"));
    }

    #[test]
    fn cursor_mcp_server_written_to_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        register_mcp_server(&path, "postgres", "uvx mcp-server-postgres");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["postgres"]["command"], "gate");
        let args = v["mcpServers"]["postgres"]["args"].as_array().unwrap();
        assert_eq!(args[0], "mcp");
        assert_eq!(args[1], "--name");
        assert_eq!(args[2], "postgres");
        assert_eq!(args[3], "--");
        assert_eq!(args[4], "uvx");
    }

    #[test]
    fn cursor_hooks_path_global_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = cursor_hooks_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.cursor/hooks.json"));
    }

    #[test]
    fn cursor_hooks_path_project_is_relative() {
        let path = cursor_hooks_path("project").unwrap();
        assert_eq!(path, PathBuf::from(".cursor/hooks.json"));
    }

    // ── codex init ────────────────────────────────────────────────────────────

    fn tmp_codex_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".codex/hooks.json");
        (dir, path)
    }

    #[test]
    fn codex_creates_hook_file_from_scratch() {
        let (_dir, path) = tmp_codex_path();
        run_codex_with_path(&path, "global");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr[0]["hooks"][0]["command"].as_str().unwrap(),
            CODEX_HOOK_COMMAND
        );
        assert_eq!(arr[0]["matcher"].as_str().unwrap(), "^Bash$");
    }

    #[test]
    fn codex_idempotent_on_second_run() {
        let (_dir, path) = tmp_codex_path();
        run_codex_with_path(&path, "global");
        run_codex_with_path(&path, "global");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        let gate_count = arr
            .iter()
            .filter(|e| {
                e.get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|h| {
                        h.iter()
                            .any(|x| x["command"].as_str() == Some(CODEX_HOOK_COMMAND))
                    })
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(gate_count, 1);
    }

    #[test]
    fn codex_preserves_existing_hooks() {
        let (_dir, path) = tmp_codex_path();
        let initial = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "^Edit$", "hooks": [{ "type": "command", "command": "other-hook" }] }
                ]
            }
        });
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_codex_with_path(&path, "global");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let cmds: Vec<&str> = arr
            .iter()
            .filter_map(|e| e["hooks"][0]["command"].as_str())
            .collect();
        assert!(cmds.contains(&"other-hook"));
        assert!(cmds.contains(&CODEX_HOOK_COMMAND));
    }

    #[test]
    fn codex_replaces_absolute_path_variant() {
        let (_dir, path) = tmp_codex_path();
        let initial = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "^Bash$", "hooks": [{ "type": "command", "command": "/usr/local/bin/gate hook --format codex" }] }
                ]
            }
        });
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_codex_with_path(&path, "global");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr[0]["hooks"][0]["command"].as_str().unwrap(),
            CODEX_HOOK_COMMAND
        );
    }

    #[test]
    fn codex_matcher_is_caret_bash_dollar() {
        let (_dir, path) = tmp_codex_path();
        run_codex_with_path(&path, "global");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr[0]["matcher"].as_str().unwrap(), "^Bash$");
    }

    #[test]
    fn codex_hooks_path_global_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = codex_hooks_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.codex/hooks.json"));
    }

    #[test]
    fn codex_hooks_path_project_is_relative() {
        let path = codex_hooks_path("project").unwrap();
        assert_eq!(path, PathBuf::from(".codex/hooks.json"));
    }

    #[test]
    fn is_gate_hook_variant_matches_codex_format() {
        assert!(is_gate_hook_variant(CODEX_HOOK_COMMAND));
        assert!(is_gate_hook_variant(
            "/usr/local/bin/gate hook --format codex"
        ));
    }

    // ── codex MCP registration ────────────────────────────────────────────────

    fn tmp_codex_config() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        (dir, path)
    }

    fn read_toml(path: &PathBuf) -> toml_edit::DocumentMut {
        let contents = std::fs::read_to_string(path).unwrap();
        contents.parse::<toml_edit::DocumentMut>().unwrap()
    }

    #[test]
    fn codex_config_path_global_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = codex_config_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.codex/config.toml"));
    }

    #[test]
    fn codex_config_path_project_is_relative() {
        let path = codex_config_path("project").unwrap();
        assert_eq!(path, PathBuf::from(".codex/config.toml"));
    }

    #[test]
    fn codex_register_creates_new_file() {
        let (_dir, path) = tmp_codex_config();
        register_mcp_server_codex(&path, "postgres", "uvx mcp-server-postgres");
        let doc = read_toml(&path);
        assert_eq!(
            doc["mcp_servers"]["postgres"]["command"].as_str().unwrap(),
            "gate"
        );
        let args = doc["mcp_servers"]["postgres"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "mcp",
                "--name",
                "postgres",
                "--",
                "uvx",
                "mcp-server-postgres"
            ]
        );
    }

    #[test]
    fn codex_register_preserves_existing_servers() {
        let (_dir, path) = tmp_codex_config();
        let initial = "[mcp_servers.github]\ncommand = \"npx\"\nargs = [\"@mcp/github\"]\n";
        std::fs::write(&path, initial).unwrap();
        register_mcp_server_codex(&path, "postgres", "uvx mcp-server-postgres");
        let doc = read_toml(&path);
        assert_eq!(
            doc["mcp_servers"]["github"]["command"].as_str().unwrap(),
            "npx"
        );
        assert_eq!(
            doc["mcp_servers"]["postgres"]["command"].as_str().unwrap(),
            "gate"
        );
    }

    #[test]
    fn codex_register_overwrites_same_name() {
        let (_dir, path) = tmp_codex_config();
        register_mcp_server_codex(&path, "postgres", "uvx mcp-server-postgres --old");
        register_mcp_server_codex(&path, "postgres", "uvx mcp-server-postgres --new");
        let doc = read_toml(&path);
        let args = doc["mcp_servers"]["postgres"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>();
        assert!(args.contains(&"--new"));
        assert!(!args.contains(&"--old"));
    }

    #[test]
    fn codex_register_multi_word_cmd_split_into_args() {
        let (_dir, path) = tmp_codex_config();
        register_mcp_server_codex(&path, "pg", "uvx mcp-server-postgres --db mydb");
        let doc = read_toml(&path);
        let args = doc["mcp_servers"]["pg"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>();
        // mcp, --name, pg, --, uvx, mcp-server-postgres, --db, mydb
        assert_eq!(args.len(), 8);
        assert_eq!(args[6], "--db");
        assert_eq!(args[7], "mydb");
    }

    // ── codex MCP wrap ────────────────────────────────────────────────────────

    #[test]
    fn codex_wrap_dry_run_leaves_file_untouched() {
        let (_dir, path) = tmp_codex_config();
        let initial =
            "[mcp_servers.postgres]\ncommand = \"uvx\"\nargs = [\"mcp-server-postgres\"]\n";
        std::fs::write(&path, initial).unwrap();
        wrap_mcp_codex(&path, None, false);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), initial);
    }

    #[test]
    fn codex_wrap_apply_rewrites_command_and_args() {
        let (_dir, path) = tmp_codex_config();
        std::fs::write(
            &path,
            "[mcp_servers.postgres]\ncommand = \"uvx\"\nargs = [\"mcp-server-postgres\"]\n",
        )
        .unwrap();
        wrap_mcp_codex(&path, None, true);
        let doc = read_toml(&path);
        assert_eq!(
            doc["mcp_servers"]["postgres"]["command"].as_str().unwrap(),
            "gate"
        );
        let args = doc["mcp_servers"]["postgres"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "mcp",
                "--name",
                "postgres",
                "--",
                "uvx",
                "mcp-server-postgres"
            ]
        );
    }

    #[test]
    fn codex_wrap_preserves_env_and_other_keys() {
        let (_dir, path) = tmp_codex_config();
        std::fs::write(
            &path,
            "[mcp_servers.pg]\ncommand = \"uvx\"\nargs = [\"mcp-server-postgres\"]\n\
             [mcp_servers.pg.env]\nPGHOST = \"localhost\"\n",
        )
        .unwrap();
        wrap_mcp_codex(&path, None, true);
        let doc = read_toml(&path);
        assert_eq!(
            doc["mcp_servers"]["pg"]["env"]["PGHOST"].as_str().unwrap(),
            "localhost"
        );
        assert_eq!(
            doc["mcp_servers"]["pg"]["command"].as_str().unwrap(),
            "gate"
        );
    }

    #[test]
    fn codex_wrap_skips_already_proxied() {
        let (_dir, path) = tmp_codex_config();
        std::fs::write(
            &path,
            "[mcp_servers.already]\ncommand = \"gate\"\nargs = [\"mcp\", \"--\", \"uvx\", \"x\"]\n\
             [mcp_servers.new]\ncommand = \"uvx\"\nargs = [\"mcp-server-y\"]\n",
        )
        .unwrap();
        wrap_mcp_codex(&path, None, true);
        let doc = read_toml(&path);
        // already-proxied unchanged
        let already_args = doc["mcp_servers"]["already"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>();
        assert_eq!(already_args[2], "uvx");
        // new one wrapped
        assert_eq!(
            doc["mcp_servers"]["new"]["command"].as_str().unwrap(),
            "gate"
        );
    }

    #[test]
    fn codex_wrap_skips_http_server_no_command() {
        let (_dir, path) = tmp_codex_config();
        std::fs::write(
            &path,
            "[mcp_servers.myhttp]\nurl = \"https://api.example.com/mcp\"\n\
             bearer_token_env_var = \"TOKEN\"\n",
        )
        .unwrap();
        // apply=true but the only server is HTTP — file should not change
        let before = std::fs::read_to_string(&path).unwrap();
        wrap_mcp_codex(&path, None, true);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn codex_wrap_comments_survive_round_trip() {
        let (_dir, path) = tmp_codex_config();
        let initial = "# project config\n\
             [mcp_servers.postgres]\n\
             command = \"uvx\"\n\
             args = [\"mcp-server-postgres\"]\n";
        std::fs::write(&path, initial).unwrap();
        wrap_mcp_codex(&path, None, true);
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(
            result.contains("# project config"),
            "comment must survive: {result}"
        );
    }

    #[test]
    fn is_codex_gate_mcp_proxy_detected() {
        let toml = "[s]\ncommand = \"gate\"\nargs = [\"mcp\", \"--\", \"uvx\"]\n";
        let doc = toml.parse::<toml_edit::DocumentMut>().unwrap();
        assert!(is_codex_gate_mcp_proxy(&doc["s"]));
    }

    #[test]
    fn is_codex_gate_mcp_proxy_not_detected_for_other_command() {
        let toml = "[s]\ncommand = \"uvx\"\nargs = [\"mcp-server-postgres\"]\n";
        let doc = toml.parse::<toml_edit::DocumentMut>().unwrap();
        assert!(!is_codex_gate_mcp_proxy(&doc["s"]));
    }

    // ── gemini init ───────────────────────────────────────────────────────────

    fn tmp_gemini_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        (dir, path)
    }

    #[test]
    fn gemini_settings_path_global_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = gemini_settings_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.gemini/settings.json"));
    }

    #[test]
    fn gemini_settings_path_project_is_relative() {
        let path = gemini_settings_path("project").unwrap();
        assert_eq!(path, PathBuf::from(".gemini/settings.json"));
    }

    #[test]
    fn gemini_creates_hook_from_empty_file() {
        let (_dir, path) = tmp_gemini_path();
        run_gemini_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["BeforeTool"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr[0]["hooks"][0]["command"].as_str().unwrap(),
            GEMINI_HOOK_COMMAND
        );
        assert_eq!(arr[0]["matcher"].as_str().unwrap(), "^run_shell_command$");
    }

    #[test]
    fn gemini_idempotent_on_second_run() {
        let (_dir, path) = tmp_gemini_path();
        run_gemini_with_path(&path);
        run_gemini_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["BeforeTool"].as_array().unwrap();
        let gate_count = arr.iter().filter(|e| gemini_entry_has_gate_hook(e)).count();
        assert_eq!(gate_count, 1);
    }

    #[test]
    fn gemini_preserves_existing_non_gate_entry() {
        let (_dir, path) = tmp_gemini_path();
        let initial = json!({
            "hooks": {
                "BeforeTool": [
                    { "matcher": "^read_file$", "hooks": [{ "type": "command", "command": "other-hook" }] }
                ]
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_gemini_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["BeforeTool"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let cmds: Vec<&str> = arr
            .iter()
            .filter_map(|e| e["hooks"][0]["command"].as_str())
            .collect();
        assert!(cmds.contains(&"other-hook"));
        assert!(cmds.contains(&GEMINI_HOOK_COMMAND));
    }

    #[test]
    fn gemini_replaces_absolute_path_variant() {
        let (_dir, path) = tmp_gemini_path();
        let initial = json!({
            "hooks": {
                "BeforeTool": [
                    { "matcher": "^run_shell_command$", "hooks": [{ "type": "command", "command": "/usr/local/bin/gate hook --format gemini" }] }
                ]
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_gemini_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["BeforeTool"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr[0]["hooks"][0]["command"].as_str().unwrap(),
            GEMINI_HOOK_COMMAND
        );
    }

    #[test]
    fn gemini_preserves_mcp_servers_when_hook_added() {
        let (_dir, path) = tmp_gemini_path();
        let initial = json!({
            "mcpServers": {
                "postgres": { "command": "uvx", "args": ["mcp-server-postgres"], "env": {} }
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_gemini_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            v["mcpServers"]["postgres"].is_object(),
            "mcpServers must survive hook install"
        );
        assert!(v["hooks"]["BeforeTool"].as_array().unwrap().len() == 1);
    }

    #[test]
    fn gemini_register_mcp_preserves_hooks_key() {
        let (_dir, path) = tmp_gemini_path();
        let initial = json!({
            "hooks": {
                "BeforeTool": [
                    { "matcher": "^run_shell_command$", "hooks": [{ "type": "command", "command": GEMINI_HOOK_COMMAND }] }
                ]
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        register_mcp_server(&path, "postgres", "uvx mcp-server-postgres");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["postgres"]["command"], "gate");
        assert!(
            v["hooks"]["BeforeTool"].as_array().is_some(),
            "hooks must survive MCP register"
        );
    }

    #[test]
    fn gemini_entry_has_gate_hook_detects_exact() {
        let entry = json!({ "matcher": "^run_shell_command$", "hooks": [{ "type": "command", "command": GEMINI_HOOK_COMMAND }] });
        assert!(gemini_entry_has_gate_hook(&entry));
    }

    #[test]
    fn gemini_entry_has_gate_hook_detects_absolute_path() {
        let entry = json!({ "matcher": "^run_shell_command$", "hooks": [{ "type": "command", "command": "/usr/local/bin/gate hook --format gemini" }] });
        assert!(gemini_entry_has_gate_hook(&entry));
    }

    #[test]
    fn gemini_entry_has_gate_hook_ignores_other_entries() {
        let entry = json!({ "matcher": "^read_file$", "hooks": [{ "type": "command", "command": "other-hook" }] });
        assert!(!gemini_entry_has_gate_hook(&entry));
    }

    // ── codebuddy hook tests ─────────────────────────────────────────────────

    fn tmp_codebuddy_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        (dir, path)
    }

    #[test]
    fn codebuddy_settings_path_global_uses_home() {
        let _lock = HOME_LOCK.lock().unwrap();
        let saved = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", "/test/home") };
        let path = codebuddy_settings_path("global").unwrap();
        match saved {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(path, PathBuf::from("/test/home/.codebuddy/settings.json"));
    }

    #[test]
    fn codebuddy_settings_path_project_is_relative() {
        let path = codebuddy_settings_path("project").unwrap();
        assert_eq!(path, PathBuf::from(".codebuddy/settings.json"));
    }

    #[test]
    fn codebuddy_creates_hook_from_empty_file() {
        let (_dir, path) = tmp_codebuddy_path();
        run_codebuddy_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr[0]["hooks"][0]["command"].as_str().unwrap(),
            CODEBUDDY_HOOK_COMMAND
        );
        assert_eq!(arr[0]["matcher"].as_str().unwrap(), "Bash");
    }

    #[test]
    fn codebuddy_idempotent_on_second_run() {
        let (_dir, path) = tmp_codebuddy_path();
        run_codebuddy_with_path(&path);
        run_codebuddy_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        let gate_count = arr.iter().filter(|e| entry_has_gate_hook(e)).count();
        assert_eq!(gate_count, 1);
    }

    #[test]
    fn codebuddy_replaces_absolute_path_variant() {
        let (_dir, path) = tmp_codebuddy_path();
        let initial = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "/usr/local/bin/gate hook --format codebuddy" }] }
                ]
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_codebuddy_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr[0]["hooks"][0]["command"].as_str().unwrap(),
            CODEBUDDY_HOOK_COMMAND
        );
    }

    #[test]
    fn codebuddy_preserves_existing_non_gate_entry() {
        let (_dir, path) = tmp_codebuddy_path();
        let initial = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Write", "hooks": [{ "type": "command", "command": "other-hook" }] }
                ]
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        run_codebuddy_with_path(&path);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let cmds: Vec<&str> = arr
            .iter()
            .filter_map(|e| e["hooks"][0]["command"].as_str())
            .collect();
        assert!(cmds.contains(&"other-hook"));
        assert!(cmds.contains(&CODEBUDDY_HOOK_COMMAND));
    }

    // ── team config discovery (gate config --sync) ──────────────────────────

    fn seed_existing_team_config(dir: &tempfile::TempDir) -> PathBuf {
        let gate_dir = dir.path().join(".gate");
        std::fs::create_dir_all(&gate_dir).unwrap();
        let path = gate_dir.join("config.yaml");
        std::fs::write(&path, "min_gate_version: \"9.9.9\"\n").unwrap();
        path
    }

    #[test]
    fn find_source_prefers_nested_gate_dir_over_bare_file() {
        let dir = tempfile::tempdir().unwrap();
        seed_existing_team_config(&dir);
        std::fs::write(
            dir.path().join("config.yaml"),
            "min_gate_version: \"1.1.1\"\n",
        )
        .unwrap();
        let found = find_team_config_source(dir.path()).unwrap();
        assert_eq!(found, dir.path().join(".gate").join("config.yaml"));
    }

    #[test]
    fn find_source_falls_back_to_bare_config_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, "min_gate_version: \"1.1.1\"\n").unwrap();
        assert_eq!(find_team_config_source(dir.path()), Some(path));
    }

    #[test]
    fn find_source_walks_up_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = seed_existing_team_config(&dir);
        let child = dir.path().join("nested/deeper");
        std::fs::create_dir_all(&child).unwrap();
        assert_eq!(find_team_config_source(&child), Some(path));
    }

    #[test]
    fn find_source_none_when_nothing_present() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find_team_config_source(dir.path()), None);
    }

    #[test]
    fn sync_scaffold_leaves_existing_gate_config_alone() {
        with_personal_config("", |_personal_path| {
            let dir = tempfile::tempdir().unwrap();
            let path = seed_existing_team_config(&dir);
            sync_team_config(dir.path());
            let contents = std::fs::read_to_string(&path).unwrap();
            assert_eq!(contents, "min_gate_version: \"9.9.9\"\n");
        });
    }

    #[test]
    fn sync_creates_nothing_when_no_team_config_found() {
        // Creating a team config is `gate export`'s job, not `--sync`'s — sync
        // only picks up what's already there.
        with_personal_config("", |personal_path| {
            let dir = tempfile::tempdir().unwrap();
            let before = std::fs::read_to_string(personal_path).unwrap();
            sync_team_config(dir.path());
            assert!(!dir.path().join(".gate").exists());
            assert!(!dir.path().join("config.yaml").exists());
            let after = std::fs::read_to_string(personal_path).unwrap();
            assert_eq!(before, after);
        });
    }

    #[test]
    fn sync_uses_bare_config_yaml_without_creating_gate_dir() {
        with_personal_config("", |personal_path| {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(
                dir.path().join("config.yaml"),
                "tools:\n  tkpsql:\n    sql_arg: \"--sql\"\n",
            )
            .unwrap();
            sync_team_config(dir.path());
            assert!(!dir.path().join(".gate").exists());
            let contents = std::fs::read_to_string(personal_path).unwrap();
            let parsed: common::config::Config = serde_yaml::from_str(&contents).unwrap();
            assert!(parsed.tools.contains_key("tkpsql"));
        });
    }

    // ── gate export ────────────────────────────────────────────────────────

    fn sample_personal_config() -> common::config::Config {
        use common::config::{Config, ToolConfig};
        use std::collections::HashMap;
        let mut tools = HashMap::new();
        tools.insert(
            "tkpsql".to_string(),
            ToolConfig {
                sql_arg: Some("--sql".to_string()),
                json_tool: None,
                json_sql_path: None,
                pipe: None,
                extra_args: vec![],
            },
        );
        let mut config = Config {
            tools,
            ..Config::default()
        };
        config.pii.confidence_threshold = 0.65;
        config.pii.column_name_boost = 0.2;
        config.pii.column_denylist = vec!["secret_token".to_string()];
        config.pii.column_allowlist = vec!["employee_id".to_string()];
        config
    }

    #[test]
    fn team_config_from_personal_exports_tightening_fields() {
        let dir = tempfile::tempdir().unwrap();
        let personal = sample_personal_config();
        let path = dir.path().join("config.yaml");
        write_team_config_from_personal(&path, &personal);
        let contents = std::fs::read_to_string(&path).unwrap();
        let parsed: common::config::ProjectConfig = serde_yaml::from_str(&contents).unwrap();
        assert!(parsed.tools.contains_key("tkpsql"));
        assert_eq!(parsed.pii.confidence_threshold, Some(0.65));
        assert_eq!(parsed.pii.column_name_boost, Some(0.2));
        assert_eq!(parsed.pii.column_denylist, vec!["secret_token"]);
    }

    #[test]
    fn team_config_from_personal_exports_allowlist_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let personal = sample_personal_config();
        let path = dir.path().join("config.yaml");
        write_team_config_from_personal(&path, &personal);
        let contents = std::fs::read_to_string(&path).unwrap();
        let parsed: common::config::ProjectConfig = serde_yaml::from_str(&contents).unwrap();
        assert_eq!(parsed.pii.column_allowlist, vec!["employee_id"]);
    }

    #[test]
    fn export_overwrites_existing_config_yaml() {
        // gate export always overwrites — it's meant to be git-tracked, so an
        // unwanted overwrite is recoverable via git as long as it was committed.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, "min_gate_version: \"9.9.9\"\n").unwrap();
        write_team_config_from_personal(&path, &sample_personal_config());
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_ne!(contents, "min_gate_version: \"9.9.9\"\n");
        assert!(contents.contains("tkpsql"));
    }

    // ── gate init: merge project config into personal config ─────────────────

    fn with_personal_config<F: FnOnce(&std::path::Path)>(initial_yaml: &str, f: F) {
        let _guard = GATE_CONFIG_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, initial_yaml).unwrap();
        unsafe { std::env::set_var("GATE_CONFIG", &path) };
        f(&path);
        unsafe { std::env::remove_var("GATE_CONFIG") };
    }

    fn write_project_config(dir: &tempfile::TempDir, yaml: &str) {
        let gate_dir = dir.path().join(".gate");
        std::fs::create_dir_all(&gate_dir).unwrap();
        std::fs::write(gate_dir.join("config.yaml"), yaml).unwrap();
    }

    #[test]
    fn merge_adds_new_tools_and_raises_threshold() {
        with_personal_config("pii:\n  confidence_threshold: 0.5\n", |personal_path| {
            let dir = tempfile::tempdir().unwrap();
            write_project_config(
                &dir,
                "tools:\n  tkpsql:\n    sql_arg: \"--sql\"\npii:\n  confidence_threshold: 0.9\n",
            );
            merge_project_into_personal(&dir.path().join(".gate").join("config.yaml"));
            let contents = std::fs::read_to_string(personal_path).unwrap();
            let parsed: common::config::Config = serde_yaml::from_str(&contents).unwrap();
            assert!(parsed.tools.contains_key("tkpsql"));
            assert_eq!(parsed.pii.confidence_threshold, 0.9);
        });
    }

    #[test]
    fn merge_never_lowers_personal_threshold() {
        with_personal_config("pii:\n  confidence_threshold: 0.9\n", |personal_path| {
            let dir = tempfile::tempdir().unwrap();
            write_project_config(&dir, "pii:\n  confidence_threshold: 0.3\n");
            merge_project_into_personal(&dir.path().join(".gate").join("config.yaml"));
            // Nothing changed (0.3 does not raise 0.9), so the file must be
            // untouched, not rewritten with a lowered value.
            let contents = std::fs::read_to_string(personal_path).unwrap();
            assert_eq!(contents, "pii:\n  confidence_threshold: 0.9\n");
        });
    }

    #[test]
    fn merge_preserves_existing_personal_tools() {
        with_personal_config("tools:\n  mysql:\n    sql_arg: \"-e\"\n", |personal_path| {
            let dir = tempfile::tempdir().unwrap();
            write_project_config(&dir, "tools:\n  tkpsql:\n    sql_arg: \"--sql\"\n");
            merge_project_into_personal(&dir.path().join(".gate").join("config.yaml"));
            let contents = std::fs::read_to_string(personal_path).unwrap();
            let parsed: common::config::Config = serde_yaml::from_str(&contents).unwrap();
            assert!(
                parsed.tools.contains_key("mysql"),
                "existing personal tool must survive"
            );
            assert!(
                parsed.tools.contains_key("tkpsql"),
                "new project tool must be added"
            );
        });
    }

    #[test]
    fn merge_adds_column_allowlist_entries_by_explicit_design() {
        // Per an explicit, twice-confirmed product decision, gate init's personal
        // merge DOES include column_allowlist (unlike gate export, this is the one
        // place a project's allowlist becomes global to the developer). Covered so
        // any accidental change to this behavior is a visible diff.
        with_personal_config("", |personal_path| {
            let dir = tempfile::tempdir().unwrap();
            write_project_config(&dir, "pii:\n  column_allowlist:\n    - user_id\n");
            merge_project_into_personal(&dir.path().join(".gate").join("config.yaml"));
            let contents = std::fs::read_to_string(personal_path).unwrap();
            let parsed: common::config::Config = serde_yaml::from_str(&contents).unwrap();
            assert_eq!(parsed.pii.column_allowlist, vec!["user_id"]);
        });
    }

    #[test]
    fn merge_is_noop_when_nothing_new() {
        with_personal_config(
            "tools:\n  tkpsql:\n    sql_arg: \"--sql\"\npii:\n  confidence_threshold: 0.9\n",
            |personal_path| {
                let dir = tempfile::tempdir().unwrap();
                write_project_config(
                    &dir,
                    "tools:\n  tkpsql:\n    sql_arg: \"--sql\"\npii:\n  confidence_threshold: 0.5\n",
                );
                let before = std::fs::read_to_string(personal_path).unwrap();
                merge_project_into_personal(&dir.path().join(".gate").join("config.yaml"));
                let after = std::fs::read_to_string(personal_path).unwrap();
                assert_eq!(
                    before, after,
                    "file must not be rewritten when nothing changed"
                );
            },
        );
    }

    #[test]
    fn merge_skips_gracefully_on_malformed_project_config() {
        with_personal_config(
            "tools:\n  tkpsql:\n    sql_arg: \"--sql\"\n",
            |personal_path| {
                let dir = tempfile::tempdir().unwrap();
                write_project_config(&dir, "pii: {bad: yaml: :: :");
                let before = std::fs::read_to_string(personal_path).unwrap();
                merge_project_into_personal(&dir.path().join(".gate").join("config.yaml"));
                let after = std::fs::read_to_string(personal_path).unwrap();
                assert_eq!(
                    before, after,
                    "malformed project config must not touch personal config"
                );
            },
        );
    }
}
