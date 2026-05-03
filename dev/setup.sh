#!/usr/bin/env bash
# One-command dev setup: start Postgres, wait for it, build the redact binary.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEV="$ROOT/dev"
BIN="$ROOT/target/release"

# On macOS, libpq is keg-only — add it to PATH if present but not linked.
for candidate in /opt/homebrew/opt/libpq/bin /usr/local/opt/libpq/bin; do
  if [[ -x "$candidate/psql" ]]; then
    export PATH="$candidate:$PATH"
    break
  fi
done
if ! command -v psql >/dev/null 2>&1; then
  echo "error: psql not found." >&2
  echo "       brew install libpq" >&2
  exit 1
fi

# Detect the available Docker Compose command (V2 plugin vs standalone binary).
# With Colima, install the standalone binary: brew install docker-compose
if docker compose version >/dev/null 2>&1; then
  DC="docker compose"
elif command -v docker-compose >/dev/null 2>&1; then
  DC="docker-compose"
else
  echo "error: neither 'docker compose' nor 'docker-compose' found." >&2
  echo "" >&2
  echo "  Using Colima?  brew install docker-compose" >&2
  echo "  Using Docker Desktop?  It should be bundled — try reinstalling." >&2
  exit 1
fi

echo "==> Starting PostgreSQL..."
(cd "$DEV" && $DC up -d)

echo -n "==> Waiting for PostgreSQL to be ready"
until (cd "$DEV" && $DC exec -T postgres \
    pg_isready -U redact -d redact_demo) >/dev/null 2>&1; do
  printf '.'
  sleep 1
done
echo " ready."

echo "==> Building redact (release)..."
cargo build --release --manifest-path "$ROOT/Cargo.toml" -q
echo "    $BIN/redact"

cat <<EOF

==> Setup complete. Run the following in your shell, then follow the steps:

    export PATH="$BIN:$DEV:\$PATH"
    export REDACT_CONFIG="$DEV/config.yaml"

Step 1 — install the hook in Claude Code (run once, outside Claude Code):

    redact init

Step 2 — verify the config is valid:

    redact validate
    redact list

Step 3 — manual smoke test (no hook needed):

    # Raw output — PII visible
    psql-json --sql "SELECT id, first_name, email, ssn, credit_card FROM users"

    # Through redact — PII replaced
    redact run -- psql-json --sql "SELECT id, first_name, email, ssn, credit_card FROM users"

    # SELECT * is rejected (wildcard_policy: reject)
    redact run -- psql-json --sql "SELECT * FROM users"

Step 4 — full hook demo inside Claude Code:
    Restart Claude Code so the hook takes effect, then ask:
    "Run psql-json --sql 'SELECT id, first_name, email, ssn, credit_card FROM users'"
    The hook intercepts the call transparently; Claude sees only the redacted JSON.

EOF
