use std::collections::BTreeMap;
use std::process::Stdio;

use common::config::Config;
use common::error::exit_with_error;
use common::harness::is_agent_harness;
use common::patterns::classify_column;

/// Run the scan subcommand: introspect a database schema and report PII exposure.
pub fn run(tool_name: &str, schema: Option<&str>) {
    if is_agent_harness() {
        exit_with_error("gate scan cannot be run inside an agent harness.");
    }

    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => exit_with_error(&format!(
            "failed to load config: {e}. Run `gate config --init-only` to create a starter config."
        )),
    };

    // Look up the tool in config
    let tool_cfg = match config.tools.get(tool_name) {
        Some(cfg) => cfg,
        None => exit_with_error(&format!(
            "tool '{tool_name}' is not configured. Run 'gate list' to see configured tools."
        )),
    };

    let sql_arg = match &tool_cfg.sql_arg {
        Some(arg) => arg,
        None => exit_with_error(&format!(
            "tool '{tool_name}' has no sql_arg configured; gate scan requires a SQL-capable tool."
        )),
    };

    // Determine schema (use provided, or default by tool type)
    let resolved_schema = schema.unwrap_or_else(|| default_schema_for_tool(tool_name));

    // Build the introspection SQL
    let sql = build_schema_sql(tool_name, resolved_schema);

    // Spawn the tool directly (bypass Gate 2) with the introspection query
    let output = spawn_tool(tool_name, sql_arg, &sql);

    // Parse the tool's JSON output to extract (table_name, column_name) pairs
    let pairs = match parse_columnar_json(&output) {
        Ok(p) => p,
        Err(e) => exit_with_error(&e),
    };

    if pairs.is_empty() {
        println!("No columns returned — check the schema name and tool configuration.");
        std::process::exit(0);
    }

    // Classify each column and aggregate results
    let stats = aggregate_by_category(&pairs);

    // Render the report
    print_report(tool_name, resolved_schema, &pairs, &stats);

    // Exit code: 0 if no PII found, 1 if any PII exists
    let has_pii = stats.iter().any(|(category, _)| category != "No PII");
    std::process::exit(if has_pii { 1 } else { 0 });
}

/// Build the introspection SQL query for a given tool and schema.
fn build_schema_sql(tool_basename: &str, schema: &str) -> String {
    match tool_basename {
        "tkdbr" | "databricks" => {
            format!(
                "SELECT table_name, column_name FROM SYSTEM.INFORMATION_SCHEMA.COLUMNS \
                 WHERE table_schema = '{}' ORDER BY table_name, column_name",
                schema
            )
        }
        "tkmsql" | "sqlcmd" => {
            format!(
                "SELECT TABLE_NAME, COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS \
                 WHERE TABLE_SCHEMA = '{}' ORDER BY TABLE_NAME, COLUMN_NAME",
                schema
            )
        }
        _ => {
            // PostgreSQL, generic ANSI
            format!(
                "SELECT table_name, column_name FROM information_schema.columns \
                 WHERE table_schema = '{}' ORDER BY table_name, column_name",
                schema
            )
        }
    }
}

/// Determine the default schema for a given tool.
fn default_schema_for_tool(tool_basename: &str) -> &'static str {
    match tool_basename {
        "tkmsql" | "sqlcmd" => "dbo",
        "tkdbr" | "databricks" => "default",
        _ => "public",
    }
}

/// Spawn the tool directly with the introspection SQL, bypassing Gate 2.
/// Returns the tool's stdout as a string.
fn spawn_tool(tool_name: &str, sql_arg: &str, sql: &str) -> String {
    let args = vec![format!("{sql_arg}={sql}")];

    let output = match std::process::Command::new(tool_name)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
    {
        Ok(o) => o,
        Err(e) => exit_with_error(&format!("{tool_name}: {e}")),
    };

    // Non-zero exit: forward to user and exit
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(2));
    }

    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Parse columnar JSON output to extract (table_name, column_name) pairs.
/// Supports both lowercase and uppercase header names (case-insensitive).
fn parse_columnar_json(json_str: &str) -> Result<Vec<(String, String)>, String> {
    let value: serde_json::Value = match serde_json::from_str(json_str.trim()) {
        Ok(v) => v,
        Err(_) => {
            return Err(
                "tool output is not JSON — check the tool is configured correctly.".to_string(),
            )
        }
    };

    // Extract columns array
    let columns = match value.get("columns") {
        Some(serde_json::Value::Array(cols)) => cols,
        _ => {
            return Err(
                "unexpected schema output shape — expected columns array in output.".to_string(),
            )
        }
    };

    // Find the indices of table_name and column_name (case-insensitive)
    let mut table_idx = None;
    let mut column_idx = None;

    for (i, col) in columns.iter().enumerate() {
        if let Some(col_str) = col.as_str() {
            let lower = col_str.to_lowercase();
            if lower == "table_name" {
                table_idx = Some(i);
            } else if lower == "column_name" {
                column_idx = Some(i);
            }
        }
    }

    let table_idx = match table_idx {
        Some(idx) => idx,
        None => {
            return Err(
                "unexpected schema output shape — expected table_name column in output."
                    .to_string(),
            )
        }
    };

    let column_idx = match column_idx {
        Some(idx) => idx,
        None => {
            return Err(
                "unexpected schema output shape — expected column_name column in output."
                    .to_string(),
            )
        }
    };

    // Extract rows
    let rows = match value.get("rows") {
        Some(serde_json::Value::Array(r)) => r,
        _ => {
            return Err(
                "unexpected schema output shape — expected rows array in output.".to_string(),
            )
        }
    };

    // Collect (table, column) pairs
    let mut pairs = Vec::new();
    for row in rows {
        if let serde_json::Value::Array(row_arr) = row {
            if let (Some(table), Some(col)) = (row_arr.get(table_idx), row_arr.get(column_idx)) {
                if let (Some(table_str), Some(col_str)) = (table.as_str(), col.as_str()) {
                    pairs.push((table_str.to_string(), col_str.to_string()));
                }
            }
        }
    }

    Ok(pairs)
}

/// Aggregation result per PII category.
struct CategoryResult {
    count: usize,
    examples: Vec<String>,
}

/// Classify each column and aggregate by PII type.
fn aggregate_by_category(pairs: &[(String, String)]) -> BTreeMap<String, CategoryResult> {
    let mut results: BTreeMap<String, CategoryResult> = BTreeMap::new();

    for (table, col) in pairs {
        let category = match classify_column(col) {
            Some(pii_type) => pii_type.to_string(),
            None => "No PII".to_string(),
        };

        let entry = results.entry(category).or_insert(CategoryResult {
            count: 0,
            examples: Vec::new(),
        });
        entry.count += 1;

        // Store up to 3 examples
        if entry.examples.len() < 3 {
            entry.examples.push(format!("{}.{}", table, col));
        }
    }

    results
}

/// Print the report to stdout.
fn print_report(
    tool_name: &str,
    schema: &str,
    pairs: &[(String, String)],
    stats: &BTreeMap<String, CategoryResult>,
) {
    let total_columns = pairs.len();
    let unique_tables = pairs
        .iter()
        .map(|(t, _)| t)
        .collect::<std::collections::HashSet<_>>()
        .len();

    // Sort categories by count descending, but keep "No PII" at the end
    let mut sorted: Vec<_> = stats.iter().collect();
    sorted.sort_by_key(|(_, result)| std::cmp::Reverse(result.count));
    let (pii_cats, no_pii_cats): (Vec<_>, Vec<_>) =
        sorted.iter().partition(|(cat, _)| cat.as_str() != "No PII");

    // Print header
    println!("Gate PII Scan — {} (schema: {})", tool_name, schema);
    println!(
        "Scanned {} columns across {} tables\n",
        total_columns, unique_tables
    );

    // Print column headers
    println!(
        "{:<18} {:<10} {:<12} Examples",
        "Category", "Columns", "% of total"
    );
    println!("{}", "─".repeat(75));

    // Print PII categories
    let total_pii: usize = pii_cats.iter().map(|(_, result)| result.count).sum();
    for (category, result) in pii_cats {
        let percentage = if total_columns > 0 {
            (result.count as f64 / total_columns as f64) * 100.0
        } else {
            0.0
        };
        let examples_str = if result.examples.len() >= 3 {
            format!("{}, {} …", result.examples[0], result.examples[1])
        } else {
            result.examples.join(", ")
        };
        println!(
            "{:<18} {:<10} {:<12.1}% {}",
            category, result.count, percentage, examples_str
        );
    }

    // Print separator and totals
    println!("{}", "─".repeat(75));

    let pii_percentage = if total_columns > 0 {
        (total_pii as f64 / total_columns as f64) * 100.0
    } else {
        0.0
    };
    println!(
        "{:<18} {:<10} {:<12.1}%",
        "Total PII", total_pii, pii_percentage
    );

    // Print No PII count
    for (category, result) in &no_pii_cats {
        let percentage = if total_columns > 0 {
            (result.count as f64 / total_columns as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "{:<18} {:<10} {:<12.1}%",
            category, result.count, percentage
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregation_counts_categories() {
        let pairs = vec![
            ("users".to_string(), "email".to_string()),
            ("users".to_string(), "first_name".to_string()),
            ("users".to_string(), "last_name".to_string()),
            ("orders".to_string(), "customer_id".to_string()),
            ("orders".to_string(), "order_date".to_string()),
        ];
        let stats = aggregate_by_category(&pairs);

        assert_eq!(stats.get("email").map(|r| r.count), Some(1));
        assert_eq!(stats.get("name").map(|r| r.count), Some(2));
        assert_eq!(stats.get("id").map(|r| r.count), Some(1));
        assert_eq!(stats.get("No PII").map(|r| r.count), Some(1));
    }

    #[test]
    fn aggregation_examples_capped_at_three() {
        let pairs = vec![
            ("t1".to_string(), "first_name".to_string()),
            ("t2".to_string(), "first_name".to_string()),
            ("t3".to_string(), "first_name".to_string()),
            ("t4".to_string(), "first_name".to_string()),
        ];
        let stats = aggregate_by_category(&pairs);
        let name_examples = &stats.get("name").unwrap().examples;
        assert_eq!(name_examples.len(), 3);
        assert!(name_examples[0].contains("first_name"));
    }

    #[test]
    fn schema_sql_postgres() {
        let sql = build_schema_sql("tkpsql", "public");
        assert!(sql.contains("information_schema.columns"));
        assert!(sql.contains("table_schema = 'public'"));
    }

    #[test]
    fn schema_sql_databricks() {
        let sql = build_schema_sql("tkdbr", "default");
        assert!(sql.contains("SYSTEM.INFORMATION_SCHEMA.COLUMNS"));
        assert!(sql.contains("table_schema = 'default'"));
    }

    #[test]
    fn schema_sql_mssql() {
        let sql = build_schema_sql("tkmsql", "dbo");
        assert!(sql.contains("INFORMATION_SCHEMA.COLUMNS"));
        assert!(sql.contains("TABLE_SCHEMA = 'dbo'"));
    }

    #[test]
    fn schema_sql_fallback() {
        let sql = build_schema_sql("unknown_tool", "myschema");
        assert!(sql.contains("information_schema.columns"));
    }

    #[test]
    fn default_schema_postgres() {
        assert_eq!(default_schema_for_tool("psql"), "public");
        assert_eq!(default_schema_for_tool("tkpsql"), "public");
    }

    #[test]
    fn default_schema_mssql() {
        assert_eq!(default_schema_for_tool("sqlcmd"), "dbo");
        assert_eq!(default_schema_for_tool("tkmsql"), "dbo");
    }

    #[test]
    fn default_schema_databricks() {
        assert_eq!(default_schema_for_tool("databricks"), "default");
        assert_eq!(default_schema_for_tool("tkdbr"), "default");
    }

    #[test]
    fn parse_columnar_json_valid() {
        let json_str = r#"{"columns": ["table_name", "column_name"], "rows": [["users", "email"], ["users", "name"]]}"#;
        let pairs = parse_columnar_json(json_str).unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("users".to_string(), "email".to_string()));
        assert_eq!(pairs[1], ("users".to_string(), "name".to_string()));
    }

    #[test]
    fn parse_columnar_json_uppercase_headers() {
        let json_str =
            r#"{"columns": ["TABLE_NAME", "COLUMN_NAME"], "rows": [["users", "email"]]}"#;
        let pairs = parse_columnar_json(json_str).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], ("users".to_string(), "email".to_string()));
    }

    #[test]
    fn parse_columnar_json_missing_table_header() {
        let json_str = r#"{"columns": ["column_name"], "rows": [["email"]]}"#;
        assert!(parse_columnar_json(json_str).is_err());
    }

    #[test]
    fn parse_columnar_json_missing_column_header() {
        let json_str = r#"{"columns": ["table_name"], "rows": [["users"]]}"#;
        assert!(parse_columnar_json(json_str).is_err());
    }

    #[test]
    fn parse_columnar_json_invalid_json() {
        let json_str = "not valid json";
        assert!(parse_columnar_json(json_str).is_err());
    }
}
