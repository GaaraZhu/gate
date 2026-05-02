use common::config::Config;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{self, Read};

pub fn run() {
    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin).unwrap_or_default();

    // Load config; on failure passthrough so a bad config never blocks every Bash command
    let tools: HashSet<String> = Config::load()
        .map(|c| c.tools.into_keys().collect())
        .unwrap_or_default();

    if let Some(output) = process(&stdin, &tools) {
        print!("{}", output);
    }
    // No output → passthrough (Claude Code allows the original command)
}

/// Returns `Some(json_string)` to rewrite, `None` to pass through unchanged.
fn process(stdin: &str, tools: &HashSet<String>) -> Option<String> {
    let hook_input: Value = serde_json::from_str(stdin).ok()?;

    let command = hook_input
        .get("tool_input")
        .and_then(|ti| ti.get("command"))
        .and_then(|c| c.as_str())?
        .to_string();

    let tokens = shell_words::split(&command)
        .ok()
        .filter(|t| !t.is_empty())?;

    let basename = std::path::Path::new(&tokens[0])
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&tokens[0])
        .to_string();

    // Loop avoidance: already routed through redact run
    if basename == "redact" && tokens.get(1).map(String::as_str) == Some("run") {
        return None;
    }

    if !tools.contains(&basename) {
        return None;
    }

    // Rewrite: preserve all tool_input fields, replace command
    let mut updated_input = hook_input["tool_input"].clone();
    if let Some(obj) = updated_input.as_object_mut() {
        obj.insert(
            "command".to_string(),
            json!(format!("redact run -- {}", command)),
        );
    }

    Some(
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow",
                "updatedInput": updated_input,
            }
        })
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn default_tools() -> HashSet<String> {
        ["tkpsql", "tkdbr", "mysql", "psql"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn make_input(command: &str) -> String {
        json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string()
    }

    #[test]
    fn passthrough_non_intercepted() {
        let tools = default_tools();
        assert!(process(&make_input("ls -la"), &tools).is_none());
        assert!(process(&make_input("grep foo bar.txt"), &tools).is_none());
    }

    #[test]
    fn rewrite_tkpsql() {
        let tools = default_tools();
        let out = process(
            &make_input("tkpsql --sql 'SELECT email FROM users'"),
            &tools,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let cmd = v["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap();
        assert!(cmd.starts_with("redact run -- tkpsql"));
        assert!(cmd.contains("SELECT email FROM users"));
    }

    #[test]
    fn rewrite_tkdbr() {
        let tools = default_tools();
        let out = process(
            &make_input("tkdbr --sql 'SELECT phone FROM contacts'"),
            &tools,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let cmd = v["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap();
        assert!(cmd.starts_with("redact run -- tkdbr"));
    }

    #[test]
    fn rewrite_mysql() {
        let tools = default_tools();
        let out = process(&make_input("mysql -e 'SELECT ssn FROM patients'"), &tools).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let cmd = v["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap();
        assert!(cmd.starts_with("redact run -- mysql"));
    }

    #[test]
    fn rewrite_psql() {
        let tools = default_tools();
        let out = process(&make_input("psql -c 'SELECT phone FROM contacts'"), &tools).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let cmd = v["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap();
        assert!(cmd.starts_with("redact run -- psql"));
    }

    #[test]
    fn loop_avoidance() {
        let tools = default_tools();
        assert!(process(&make_input("redact run -- tkpsql --sql 'SELECT 1'"), &tools).is_none());
    }

    #[test]
    fn invalid_json_passthrough() {
        let tools = default_tools();
        assert!(process("not json at all", &tools).is_none());
    }

    #[test]
    fn permission_decision_is_allow() {
        let tools = default_tools();
        let out = process(&make_input("psql -c 'SELECT phone FROM contacts'"), &tools).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecision"]
                .as_str()
                .unwrap(),
            "allow"
        );
    }

    #[test]
    fn full_path_basename_matched() {
        let tools = default_tools();
        assert!(process(
            &make_input("/usr/local/bin/tkpsql --sql 'SELECT 1'"),
            &tools
        )
        .is_some());
    }

    #[test]
    fn passthrough_when_tool_not_in_config() {
        // Empty tools set — nothing is intercepted
        let tools = HashSet::new();
        assert!(process(
            &make_input("tkpsql --sql 'SELECT email FROM users'"),
            &tools
        )
        .is_none());
    }

    #[test]
    fn command_with_quoted_sql_preserved() {
        let tools = default_tools();
        let out = process(&make_input(r#"tkpsql --sql "SELECT 'a b' FROM t""#), &tools).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let cmd = v["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap();
        // Original command should appear verbatim after "redact run -- "
        assert!(cmd.contains("SELECT 'a b'") || cmd.contains(r#"SELECT \'a b\'"#));
    }

    #[test]
    fn malformed_shell_words_passthrough() {
        let tools = default_tools();
        // Unclosed quote — shell_words::split will fail → passthrough
        let input = make_input("tkpsql --sql 'unclosed");
        assert!(process(&input, &tools).is_none());
    }

    #[test]
    fn preserves_extra_tool_input_fields() {
        let tools = default_tools();
        let input = json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {
                "command": "tkpsql --sql 'SELECT 1'",
                "restart": false
            }
        })
        .to_string();
        let out = process(&input, &tools).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        // restart field should still be present in updatedInput
        assert_eq!(
            v["hookSpecificOutput"]["updatedInput"]["restart"],
            json!(false)
        );
    }
}
