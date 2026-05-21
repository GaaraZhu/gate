# Scan queries

`gate scan` reads `TABLE_NAME, COLUMN_NAME` rows from stdin and prints a PII risk report. Any query that produces those two columns works; below are ready-to-run examples for common databases and clients.

## PostgreSQL

```bash
# toolkit-managed
tkpsql query --sql "SELECT TABLE_NAME, COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = 'public' ORDER BY TABLE_NAME, ORDINAL_POSITION" | gate scan

# native psql
psql -U <user> -h <host> -d <dbname> -c "SELECT TABLE_NAME, COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = 'public' ORDER BY TABLE_NAME, ORDINAL_POSITION" | gate scan
```

## MS SQL Server

```bash
# toolkit-managed
tkmsql query --sql "SELECT TABLE_NAME, COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS ORDER BY TABLE_NAME, ORDINAL_POSITION" | gate scan
```

### sqlcmd (native)

`gate scan` understands sqlcmd's default fixed-width table output — no extra flags needed.

**macOS / Linux:**
```bash
sqlcmd -S <server> -U <user> -d <dbname> \
  -Q "SELECT TABLE_NAME, COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS ORDER BY TABLE_NAME, ORDINAL_POSITION" \
  | gate scan
```

**Windows (cmd.exe):**

Interactive password prompts do not work through a pipe on Windows — sqlcmd writes the `Password:` prompt to stdout, which corrupts the pipe output. Avoid the prompt by providing credentials up front:

```cmd
rem Windows Authentication — no password needed
sqlcmd -S <server> -E -d <dbname> -Q "SELECT TABLE_NAME, COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS ORDER BY TABLE_NAME, ORDINAL_POSITION" | gate scan

rem SQL Authentication — set env var before the command (does not appear in command history)
set SQLCMDPASSWORD=yourpassword
sqlcmd -S <server> -U <user> -d <dbname> -Q "SELECT TABLE_NAME, COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS ORDER BY TABLE_NAME, ORDINAL_POSITION" | gate scan
```

## Databricks

```bash
# toolkit-managed
tkdbr query --conn dev --sql "SELECT TABLE_NAME, COLUMN_NAME FROM system.INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = '<schema>' ORDER BY TABLE_NAME, COLUMN_NAME" --limit 1000 | gate scan

# native databricks CLI
databricks api post /api/2.0/sql/statements --profile <profile> --json "{\"statement\": \"SELECT TABLE_NAME, COLUMN_NAME FROM system.INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = '<schema>' LIMIT 1000\", \"warehouse_id\": \"<warehouse_id>\"}" | gate scan
```

## Flags

- `--verbose` — show every detected column instead of truncating long lists
- `--json` — emit machine-readable JSON instead of the human-readable report
- `--review` — after the report, enter an interactive prompt to triage false positives and add them to the allowlist

Exit code is 1 if any PII columns are detected, so the command is safe to drop into CI audits.
