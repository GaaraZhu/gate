pub const STARTER_CONFIG: &str = r#"# redact configuration

# Tools whose Bash invocations are intercepted and piped through `redact run`.
# Only tools listed here are intercepted; everything else passes through unchanged.
tools:
  tkpsql:
    sql_arg: "--sql"   # Gate 1 parses this SQL to extract column names for targeted redaction
  tkdbr:
    sql_arg: "--sql"
  # mysql:
  #   sql_arg: ~       # mysql --json has no SQL flag; Gate 1 skipped, Gate 2 still runs
  # psql:
  #   sql_arg: ~       # psql -c has no SQL flag; Gate 1 skipped, Gate 2 still runs

pii:
  action: redact           # redact | warn | reject
  wildcard_policy: reject  # warn | reject

  # Add column names beyond the built-in denylist (email, ssn, dob, phone, npi, …)
  # column_names:
  #   - secret_token
  #   - api_key

  # Override or add PII regex patterns
  # patterns:
  #   internal_id:
  #     regex: '\bID-\d{6}\b'
  #     confidence: 0.9
"#;
