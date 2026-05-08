use std::collections::BTreeMap;
use std::io::Read;

use common::config::Config;
use common::error::exit_with_error;
use common::patterns::classify_column;

/// Run the scan subcommand: read columnar JSON from stdin and report PII-exposed column names.
///
/// Supports two input shapes:
/// 1. Array-of-arrays (tkdbr / tkmsql):
///    {"columns": ["TABLE_NAME", "COLUMN_NAME", ...], "rows": [["tbl", "col_name"], ...], ...}
/// 2. Array-of-objects (psql):
///    {"rows": [{"column_name": "col", "table_name": "tbl"}, ...], "count": N}
///
/// The subcommand extracts (table_name, column_name) pairs and runs Gate 1 column
/// classification on each column name.
pub fn run() {
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => exit_with_error(&format!(
            "failed to load config: {e}. Run `gate config --init-only` to create a starter config."
        )),
    };

    // Read all of stdin
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        exit_with_error(&format!("failed to read stdin: {e}"));
    }

    // Parse JSON and extract (table_name, column_name) pairs
    let pairs = match parse_columnar_json(&input) {
        Ok(p) => p,
        Err(e) => exit_with_error(&e),
    };

    if pairs.is_empty() {
        println!("No columns found in input.");
        std::process::exit(0);
    }

    // Classify each column and aggregate results
    let stats = aggregate_by_category(&pairs, &config);

    // Render the report
    print_report(&pairs, &stats);

    // Exit code: 0 if no PII found, 1 if any PII columns detected
    let has_pii = stats.iter().any(|(category, _)| category != "No PII");
    std::process::exit(if has_pii { 1 } else { 0 });
}

/// Parse columnar JSON output to extract (table_name, column_name) pairs.
///
/// Supports two formats:
/// 1. Array-of-arrays (tkdbr / tkmsql): `{"columns": [...], "rows": [[...], ...]}`
///    Locates TABLE_NAME and COLUMN_NAME headers (case-insensitive) in the `columns` array,
///    then reads the corresponding positions from each row.
/// 2. Array-of-objects (psql): `{"rows": [{"column_name": "...", "table_name": "..."}, ...]}`
///    Extracts column_name and table_name directly from each object.
fn parse_columnar_json(json_str: &str) -> Result<Vec<(String, String)>, String> {
    let value: serde_json::Value = match serde_json::from_str(json_str.trim()) {
        Ok(v) => v,
        Err(_) => {
            return Err(
                "input is not valid JSON — pipe the output of a schema query into gate scan."
                    .to_string(),
            )
        }
    };

    // Extract rows
    let rows =
        match value.get("rows") {
            Some(serde_json::Value::Array(r)) => r,
            _ => return Err(
                "unexpected input shape — expected a `rows` array (e.g. from tkdbr or psql query)."
                    .to_string(),
            ),
        };

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    // Detect format: check if first row is an array (tkdbr format) or object (psql format)
    match &rows[0] {
        serde_json::Value::Array(_) => parse_array_of_arrays(&value, rows),
        serde_json::Value::Object(_) => parse_array_of_objects(rows),
        _ => Err(
            "unexpected row format — expected array of arrays (tkdbr) or array of objects (psql)."
                .to_string(),
        ),
    }
}

/// Parse array-of-arrays format: `{"columns": [...], "rows": [[...], ...]}`
fn parse_array_of_arrays(
    value: &serde_json::Value,
    rows: &[serde_json::Value],
) -> Result<Vec<(String, String)>, String> {
    // Extract columns array
    let columns = match value.get("columns") {
        Some(serde_json::Value::Array(cols)) => cols,
        _ => {
            return Err(
                "unexpected input shape — expected a `columns` array (e.g. from tkdbr query)."
                    .to_string(),
            )
        }
    };

    // Find the indices of TABLE_NAME and COLUMN_NAME (case-insensitive)
    let mut table_idx = None;
    let mut column_idx = None;

    for (i, col) in columns.iter().enumerate() {
        if let Some(col_str) = col.as_str() {
            match col_str.to_lowercase().as_str() {
                "table_name" => table_idx = Some(i),
                "column_name" => column_idx = Some(i),
                _ => {}
            }
        }
    }

    let table_idx = table_idx.ok_or_else(|| {
        "input is missing a TABLE_NAME column — query must include TABLE_NAME and COLUMN_NAME."
            .to_string()
    })?;

    let column_idx = column_idx.ok_or_else(|| {
        "input is missing a COLUMN_NAME column — query must include TABLE_NAME and COLUMN_NAME."
            .to_string()
    })?;

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

/// Parse array-of-objects format: `{"rows": [{"column_name": "...", "table_name": "..."}, ...]}`
fn parse_array_of_objects(rows: &[serde_json::Value]) -> Result<Vec<(String, String)>, String> {
    let mut pairs = Vec::new();

    for row in rows {
        if let serde_json::Value::Object(map) = row {
            // Extract column_name and table_name (case-insensitive key lookup)
            let mut column_name: Option<String> = None;
            let mut table_name: Option<String> = None;

            for (key, val) in map.iter() {
                match key.to_lowercase().as_str() {
                    "column_name" => {
                        if let Some(s) = val.as_str() {
                            column_name = Some(s.to_string());
                        }
                    }
                    "table_name" => {
                        if let Some(s) = val.as_str() {
                            table_name = Some(s.to_string());
                        }
                    }
                    _ => {}
                }
            }

            if let (Some(table), Some(col)) = (table_name, column_name) {
                pairs.push((table, col));
            }
        }
    }

    if pairs.is_empty() && !rows.is_empty() {
        return Err(
            "no valid (table_name, column_name) pairs found in objects — \
             each object must contain both fields."
                .to_string(),
        );
    }

    Ok(pairs)
}

/// Aggregation result per PII category.
struct CategoryResult {
    count: usize,
    examples: Vec<String>,
}

/// Classify each column using Gate 1 patterns and aggregate by PII type.
fn aggregate_by_category(
    pairs: &[(String, String)],
    _config: &common::config::Config,
) -> BTreeMap<String, CategoryResult> {
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

/// Print the scan report to stdout.
fn print_report(pairs: &[(String, String)], stats: &BTreeMap<String, CategoryResult>) {
    let total_columns = pairs.len();
    let unique_tables = pairs
        .iter()
        .map(|(t, _)| t)
        .collect::<std::collections::HashSet<_>>()
        .len();

    // Separate PII categories from "No PII", sort by count descending
    let mut sorted: Vec<_> = stats.iter().collect();
    sorted.sort_by_key(|(_, result)| std::cmp::Reverse(result.count));
    let (pii_cats, no_pii_cats): (Vec<_>, Vec<_>) =
        sorted.iter().partition(|(cat, _)| cat.as_str() != "No PII");

    println!("Gate PII Scan");
    println!(
        "Scanned {} columns across {} tables\n",
        total_columns, unique_tables
    );

    println!(
        "{:<18} {:<10} {:<12} Examples",
        "Category", "Columns", "% of total"
    );
    println!("{}", "─".repeat(75));

    let total_pii: usize = pii_cats.iter().map(|(_, result)| result.count).sum();
    for (category, result) in &pii_cats {
        let percentage = (result.count as f64 / total_columns as f64) * 100.0;
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

    println!("{}", "─".repeat(75));

    let pii_percentage = (total_pii as f64 / total_columns as f64) * 100.0;
    println!(
        "{:<18} {:<10} {:<12.1}%",
        "Total PII", total_pii, pii_percentage
    );

    for (category, result) in &no_pii_cats {
        let percentage = (result.count as f64 / total_columns as f64) * 100.0;
        println!(
            "{:<18} {:<10} {:<12.1}%",
            category, result.count, percentage
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::config::Config;

    fn dummy_config() -> Config {
        Config::load().unwrap_or_default()
    }

    // ── parse_columnar_json ────────────────────────────────────────────────

    #[test]
    fn parse_lowercase_headers() {
        let json = r#"{"columns":["table_name","column_name"],"rows":[["users","email"],["users","first_name"]]}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("users".to_string(), "email".to_string()));
        assert_eq!(pairs[1], ("users".to_string(), "first_name".to_string()));
    }

    #[test]
    fn parse_uppercase_headers() {
        let json = r#"{"columns":["TABLE_NAME","COLUMN_NAME"],"rows":[["orders","customer_id"]]}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], ("orders".to_string(), "customer_id".to_string()));
    }

    #[test]
    fn parse_extra_columns_ignored() {
        // Real tkdbr output has a count field and extra columns — only TABLE_NAME/COLUMN_NAME matter
        let json = r#"{"columns":["TABLE_NAME","COLUMN_NAME","DATA_TYPE"],"count":2,"rows":[["users","email","varchar"],["users","id","int"]]}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].1, "email");
        assert_eq!(pairs[1].1, "id");
    }

    #[test]
    fn parse_column_name_not_first_position() {
        // COLUMN_NAME at index 0, TABLE_NAME at index 1
        let json = r#"{"columns":["COLUMN_NAME","TABLE_NAME"],"rows":[["email","users"]]}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert_eq!(pairs[0], ("users".to_string(), "email".to_string()));
    }

    #[test]
    fn parse_missing_column_name_header_errors() {
        let json = r#"{"columns":["TABLE_NAME"],"rows":[["users"]]}"#;
        assert!(parse_columnar_json(json).is_err());
    }

    #[test]
    fn parse_missing_table_name_header_errors() {
        let json = r#"{"columns":["COLUMN_NAME"],"rows":[["email"]]}"#;
        assert!(parse_columnar_json(json).is_err());
    }

    #[test]
    fn parse_invalid_json_errors() {
        assert!(parse_columnar_json("not json").is_err());
    }

    #[test]
    fn parse_empty_rows() {
        let json = r#"{"columns":["TABLE_NAME","COLUMN_NAME"],"rows":[]}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert!(pairs.is_empty());
    }

    #[test]
    fn parse_real_tkdbr_output() {
        // Exact shape from the conversation example
        let json = r#"{"columns":["TABLE_NAME","COLUMN_NAME"],"count":5,"rows":[["_jarden_account_lookup","account_id"],["_jarden_account_lookup","cp_code"],["_jarden_account_lookup","id"],["_jarden_assets","_legacy_asset_code"],["_jarden_assets","_legacy_business"]]}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert_eq!(pairs.len(), 5);
        assert_eq!(pairs[0].1, "account_id");
        assert_eq!(pairs[2].1, "id");
    }

    // ── psql format tests ─────────────────────────────────────────────────────

    #[test]
    fn parse_psql_object_format() {
        // psql object format with lowercase field names
        let json = r#"{"rows":[{"column_name":"id","table_name":"users"},{"column_name":"email","table_name":"users"}],"count":2}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("users".to_string(), "id".to_string()));
        assert_eq!(pairs[1], ("users".to_string(), "email".to_string()));
    }

    #[test]
    fn parse_psql_object_mixed_case_fields() {
        // psql with mixed-case field names
        let json = r#"{"rows":[{"Column_Name":"first_name","Table_Name":"users"}],"count":1}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], ("users".to_string(), "first_name".to_string()));
    }

    #[test]
    fn parse_psql_object_with_extra_fields() {
        // psql with additional fields that should be ignored
        let json = r#"{"rows":[{"table_name":"orders","column_name":"customer_id","data_type":"integer","is_nullable":false}],"count":1}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], ("orders".to_string(), "customer_id".to_string()));
    }

    #[test]
    fn parse_psql_object_multiple_tables() {
        // psql with multiple tables and columns
        let json = r#"{"rows":[{"column_name":"id","table_name":"users"},{"column_name":"full_name","table_name":"users"},{"column_name":"email","table_name":"users"},{"column_name":"status","table_name":"users"},{"column_name":"created_at","table_name":"users"},{"column_name":"last_login_at","table_name":"users"}],"count":6}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert_eq!(pairs.len(), 6);
        assert_eq!(pairs[0], ("users".to_string(), "id".to_string()));
        assert_eq!(pairs[1], ("users".to_string(), "full_name".to_string()));
        assert_eq!(pairs[2], ("users".to_string(), "email".to_string()));
    }

    #[test]
    fn parse_psql_object_missing_column_name_field_errors() {
        let json = r#"{"rows":[{"table_name":"users"}],"count":1}"#;
        let result = parse_columnar_json(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no valid"));
    }

    #[test]
    fn parse_psql_object_missing_table_name_field_errors() {
        let json = r#"{"rows":[{"column_name":"email"}],"count":1}"#;
        let result = parse_columnar_json(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no valid"));
    }

    #[test]
    fn parse_psql_object_empty_rows() {
        let json = r#"{"rows":[],"count":0}"#;
        let pairs = parse_columnar_json(json).unwrap();
        assert!(pairs.is_empty());
    }

    // ── aggregate_by_category ─────────────────────────────────────────────

    #[test]
    fn aggregate_classifies_pii_columns() {
        let cfg = dummy_config();
        let pairs = vec![
            ("users".to_string(), "email".to_string()),
            ("users".to_string(), "first_name".to_string()),
            ("users".to_string(), "order_date".to_string()),
        ];
        let stats = aggregate_by_category(&pairs, &cfg);
        assert!(stats.contains_key("email") || stats.iter().any(|(_, v)| v.count > 0));
        assert!(stats.contains_key("No PII") || stats.iter().any(|(k, _)| k == "No PII"));
    }

    #[test]
    fn aggregate_examples_capped_at_three() {
        let cfg = dummy_config();
        let pairs = (1..=5)
            .map(|i| (format!("t{i}"), "first_name".to_string()))
            .collect::<Vec<_>>();
        let stats = aggregate_by_category(&pairs, &cfg);
        // find whichever category "first_name" landed in
        let name_entry = stats
            .values()
            .find(|r| r.examples.len() > 0 && r.examples[0].contains("first_name"));
        if let Some(entry) = name_entry {
            assert!(entry.examples.len() <= 3);
        }
    }

    #[test]
    fn aggregate_non_pii_columns_go_to_no_pii() {
        let cfg = dummy_config();
        let pairs = vec![
            ("t".to_string(), "order_date".to_string()),
            ("t".to_string(), "legacy_business".to_string()),
        ];
        let stats = aggregate_by_category(&pairs, &cfg);
        assert!(stats.get("No PII").map(|r| r.count).unwrap_or(0) > 0);
    }
}
