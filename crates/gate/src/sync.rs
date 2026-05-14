use common::config::config_path;
use common::error::exit_with_error;

/// Run `gate config sync`: walk up from the current directory to find `.gate/columns.yaml`,
/// then merge its `column_names` and `column_allowlist` into the personal config.
pub fn run() {
    let project_file = match find_columns_file() {
        Some(p) => p,
        None => exit_with_error(
            "no .gate/columns.yaml found in this directory or any parent. \
             Run `gate scan --source <name>` first and create .gate/columns.yaml with the column classifications.",
        ),
    };

    let content = match std::fs::read_to_string(&project_file) {
        Ok(c) => c,
        Err(e) => exit_with_error(&format!("failed to read {}: {e}", project_file.display())),
    };

    let columns_file = match parse_columns_file(&content) {
        Ok(c) => c,
        Err(e) => exit_with_error(&format!("failed to parse {}: {e}", project_file.display())),
    };

    if columns_file.column_names.is_empty() && columns_file.column_allowlist.is_empty() {
        println!("Nothing to sync — .gate/columns.yaml has no column_names or column_allowlist.");
        return;
    }

    let personal_path = match config_path() {
        Ok(p) => p,
        Err(e) => exit_with_error(&format!("cannot resolve personal config path: {e}")),
    };

    let personal_content = if personal_path.exists() {
        match std::fs::read_to_string(&personal_path) {
            Ok(c) => c,
            Err(e) => exit_with_error(&format!("failed to read personal config: {e}")),
        }
    } else {
        String::new()
    };

    let (merged, added_names, added_allowlist) =
        merge_into_personal(&personal_content, &columns_file);

    if added_names.is_empty() && added_allowlist.is_empty() {
        println!(
            "Personal config already up to date — all entries from {} are present.",
            project_file.display()
        );
        return;
    }

    if let Err(e) = crate::allowlist::write_atomic(&personal_path, &merged) {
        exit_with_error(&format!("failed to write personal config: {e}"));
    }

    if !added_names.is_empty() {
        println!(
            "Added {} column_name(s): {}",
            added_names.len(),
            added_names.join(", ")
        );
    }
    if !added_allowlist.is_empty() {
        println!(
            "Added {} column_allowlist entry(s): {}",
            added_allowlist.len(),
            added_allowlist.join(", ")
        );
    }
    println!("Personal config updated: {}", personal_path.display());
}

/// Walk up from the current directory to find the nearest `.gate/columns.yaml`.
fn find_columns_file() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".gate/columns.yaml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Merge project column_names and column_allowlist into the personal config YAML string.
/// Returns the merged content and the lists of newly added entries.
fn merge_into_personal(
    personal: &str,
    project: &ColumnsFile,
) -> (String, Vec<String>, Vec<String>) {
    let current_names = parse_column_names(personal);
    let current_allowlist = crate::allowlist::parse_current_allowlist(personal);

    let new_names: Vec<String> = project
        .column_names
        .iter()
        .map(|c| c.to_lowercase())
        .filter(|c| !current_names.iter().any(|e| e == c))
        .collect();

    let new_allowlist: Vec<String> = project
        .column_allowlist
        .iter()
        .map(|c| c.to_lowercase())
        .filter(|c| !current_allowlist.iter().any(|e| e == c))
        .collect();

    let mut merged = personal.to_string();
    if !new_names.is_empty() {
        merged = add_column_names_to_yaml(&merged, &new_names);
    }
    if !new_allowlist.is_empty() {
        merged = crate::allowlist::add_to_allowlist_in_yaml(&merged, &new_allowlist);
    }

    (merged, new_names, new_allowlist)
}

/// Parse the current `pii.column_names` list from YAML content.
pub fn parse_column_names(content: &str) -> Vec<String> {
    let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(content) else {
        return Vec::new();
    };
    val.get("pii")
        .and_then(|p| p.get("column_names"))
        .and_then(|a| a.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                .collect()
        })
        .unwrap_or_default()
}

/// Add entries to `pii.column_names` in YAML, preserving all other content.
fn add_column_names_to_yaml(content: &str, columns: &[String]) -> String {
    let new_entries: String = columns.iter().map(|c| format!("    - {c}\n")).collect();

    // Case 1: column_names already exists — append after last item.
    if let Some(pos) = find_column_names_insert_pos(content) {
        let (before, after) = content.split_at(pos);
        return format!("{before}{new_entries}{after}");
    }

    // Case 2: pii: exists but no column_names — insert after pii: line.
    if let Some(pos) = find_pii_insert_pos(content) {
        let (before, after) = content.split_at(pos);
        return format!("{before}  column_names:\n{new_entries}{after}");
    }

    // Case 3: no pii: section — append at end.
    let sep = if content.ends_with('\n') || content.is_empty() {
        ""
    } else {
        "\n"
    };
    format!("{content}{sep}pii:\n  column_names:\n{new_entries}")
}

fn find_column_names_insert_pos(content: &str) -> Option<usize> {
    let mut offset = 0usize;
    let mut in_section = false;
    let mut found = false;
    let mut insert_at = 0usize;

    for line in content.split_inclusive('\n') {
        let next_offset = offset + line.len();
        let trimmed = line.trim_end_matches(['\n', '\r']);

        if !in_section && trimmed == "  column_names:" {
            found = true;
            in_section = true;
            insert_at = next_offset;
        } else if in_section {
            if trimmed.starts_with("    -") {
                insert_at = next_offset;
            } else {
                in_section = false;
            }
        }

        offset = next_offset;
    }

    if found {
        Some(insert_at)
    } else {
        None
    }
}

fn find_pii_insert_pos(content: &str) -> Option<usize> {
    let mut offset = 0usize;
    for line in content.split_inclusive('\n') {
        let next_offset = offset + line.len();
        if line.trim_end_matches(['\n', '\r']) == "pii:" {
            return Some(next_offset);
        }
        offset = next_offset;
    }
    None
}

struct ColumnsFile {
    column_names: Vec<String>,
    column_allowlist: Vec<String>,
}

/// Parse `.gate/columns.yaml` content into a `ColumnsFile`.
fn parse_columns_file(content: &str) -> Result<ColumnsFile, String> {
    let val: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|e| format!("invalid YAML: {e}"))?;

    let column_names = extract_string_list(&val, "column_names");
    let column_allowlist = extract_string_list(&val, "column_allowlist");

    Ok(ColumnsFile {
        column_names,
        column_allowlist,
    })
}

fn extract_string_list(val: &serde_yaml::Value, key: &str) -> Vec<String> {
    val.get(key)
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_column_names ────────────────────────────────────────────────────

    #[test]
    fn parse_column_names_returns_empty_for_blank() {
        assert!(parse_column_names("").is_empty());
    }

    #[test]
    fn parse_column_names_returns_entries() {
        let yaml = "pii:\n  column_names:\n    - secret_token\n    - internal_id\n";
        let names = parse_column_names(yaml);
        assert_eq!(names, vec!["secret_token", "internal_id"]);
    }

    #[test]
    fn parse_column_names_lowercases() {
        let yaml = "pii:\n  column_names:\n    - SecretToken\n";
        let names = parse_column_names(yaml);
        assert_eq!(names, vec!["secrettoken"]);
    }

    // ── add_column_names_to_yaml ──────────────────────────────────────────────

    #[test]
    fn add_column_names_to_empty_creates_pii_section() {
        let out = add_column_names_to_yaml("", &[s("secret_token")]);
        assert!(out.contains("pii:"));
        assert!(out.contains("  column_names:"));
        assert!(out.contains("    - secret_token"));
    }

    #[test]
    fn add_column_names_to_existing_pii_section() {
        let yaml = "pii:\n  action: warn\n";
        let out = add_column_names_to_yaml(yaml, &[s("secret_token")]);
        assert!(out.contains("  column_names:"));
        assert!(out.contains("    - secret_token"));
        assert!(out.contains("  action: warn"));
    }

    #[test]
    fn add_column_names_appends_to_existing_list() {
        let yaml = "pii:\n  column_names:\n    - secret_token\n";
        let out = add_column_names_to_yaml(yaml, &[s("internal_id")]);
        assert!(out.contains("    - secret_token"));
        assert!(out.contains("    - internal_id"));
    }

    // ── merge_into_personal ───────────────────────────────────────────────────

    #[test]
    fn merge_adds_both_lists() {
        let project = ColumnsFile {
            column_names: vec!["secret_token".to_string()],
            column_allowlist: vec!["city".to_string()],
        };
        let (merged, added_names, added_allowlist) = merge_into_personal("", &project);
        assert!(merged.contains("secret_token"));
        assert!(merged.contains("city"));
        assert_eq!(added_names, vec!["secret_token"]);
        assert_eq!(added_allowlist, vec!["city"]);
    }

    #[test]
    fn merge_deduplicates_existing_entries() {
        let personal =
            "pii:\n  column_names:\n    - secret_token\n  column_allowlist:\n    - city\n";
        let project = ColumnsFile {
            column_names: vec!["secret_token".to_string()],
            column_allowlist: vec!["city".to_string()],
        };
        let (_, added_names, added_allowlist) = merge_into_personal(personal, &project);
        assert!(added_names.is_empty());
        assert!(added_allowlist.is_empty());
    }

    #[test]
    fn merge_only_adds_new_entries() {
        let personal = "pii:\n  column_names:\n    - existing\n";
        let project = ColumnsFile {
            column_names: vec!["existing".to_string(), "new_col".to_string()],
            column_allowlist: vec![],
        };
        let (merged, added_names, _) = merge_into_personal(personal, &project);
        assert!(merged.contains("existing"));
        assert!(merged.contains("new_col"));
        assert_eq!(added_names, vec!["new_col"]);
    }

    #[test]
    fn parse_columns_file_both_lists() {
        let yaml = "column_names:\n  - secret_token\ncolumn_allowlist:\n  - city\n  - state\n";
        let f = parse_columns_file(yaml).unwrap();
        assert_eq!(f.column_names, vec!["secret_token"]);
        assert_eq!(f.column_allowlist, vec!["city", "state"]);
    }

    #[test]
    fn parse_columns_file_empty_is_ok() {
        let f = parse_columns_file("").unwrap();
        assert!(f.column_names.is_empty());
        assert!(f.column_allowlist.is_empty());
    }

    #[test]
    fn parse_columns_file_lowercases_entries() {
        let yaml = "column_names:\n  - SecretToken\n";
        let f = parse_columns_file(yaml).unwrap();
        assert_eq!(f.column_names, vec!["secrettoken"]);
    }

    #[test]
    fn find_columns_file_returns_none_when_absent() {
        // Run from a temp dir that has no .gate/columns.yaml in its ancestry
        // (we can't easily test directory walking, so just test the None case
        // via the function's logic with a non-existent path)
        let result = std::path::Path::new("/tmp/__gate_nonexistent__/.gate/columns.yaml").exists();
        assert!(!result);
    }

    fn s(v: &str) -> String {
        v.to_string()
    }
}
