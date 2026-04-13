#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$ROOT_DIR/.env"
POSTGRES_CONTAINER="${POSTGRES_CONTAINER:-grid-binance-postgres-1}"
API_CONTAINER="${API_CONTAINER:-grid-binance-api-server-1}"
DEFAULT_PASSWORD="${SUPER_ADMIN_PASSWORD:-pass1234}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing env file: $ENV_FILE" >&2
  exit 1
fi

set -a
source "$ENV_FILE"
set +a

SUPER_ADMIN_EMAIL="${1:-${SUPER_ADMIN_EMAILS%%,*}}"
if [[ -z "$SUPER_ADMIN_EMAIL" ]]; then
  echo "SUPER_ADMIN_EMAILS is empty" >&2
  exit 1
fi

RESET_SQL=$(cat <<'SQL'
BEGIN;
TRUNCATE TABLE
  audit_logs,
  notification_logs,
  fund_sweep_transfers,
  fund_sweep_jobs,
  deposit_transactions,
  deposit_order_queue,
  deposit_address_allocations,
  strategy_runtime_positions,
  strategy_profit_snapshots,
  strategy_orders,
  strategy_grid_levels,
  strategy_fills,
  strategy_events,
  strategy_revisions,
  strategies,
  exchange_account_trade_history,
  exchange_wallet_snapshots,
  account_profit_snapshots,
  membership_orders,
  membership_entitlements,
  telegram_bindings,
  user_exchange_symbol_metadata,
  user_exchange_credentials,
  user_exchange_accounts,
  user_sessions,
  user_totp_factors,
  password_reset_tokens,
  email_verification_tokens,
  admin_users,
  users,
  shared_sequences
RESTART IDENTITY CASCADE;
COMMIT;
SQL
)

docker exec -i "$POSTGRES_CONTAINER" psql -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" <<SQL
$RESET_SQL
SQL

docker exec -e SUPER_ADMIN_EMAIL="$SUPER_ADMIN_EMAIL" -e SUPER_ADMIN_PASSWORD="$DEFAULT_PASSWORD" -i "$API_CONTAINER" python - <<'PY'
import json
import os
import sys
import urllib.error
import urllib.request
import urllib.parse

email = os.environ["SUPER_ADMIN_EMAIL"].strip()
password = os.environ["SUPER_ADMIN_PASSWORD"].strip()
base_url = "http://127.0.0.1:8080"
issuer = urllib.parse.quote("Grid Binance", safe="")
label = urllib.parse.quote(f"Grid Binance:{email}", safe="")


def post_json(path, payload):
    request = urllib.request.Request(
        base_url + path,
        data=json.dumps(payload).encode("utf-8"),
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=20) as response:
        return json.loads(response.read().decode("utf-8"))

register = post_json("/auth/register", {"email": email, "password": password})
bootstrap = post_json("/auth/admin-bootstrap", {"email": email, "password": password})
login = post_json(
    "/auth/login",
    {"email": email, "password": password, "totp_code": bootstrap["code"]},
)

summary = {
    "email": email,
    "password": password,
    "totp_secret": bootstrap["secret"],
    "current_totp_code": bootstrap["code"],
    "otpauth_url": f"otpauth://totp/{label}?secret={bootstrap['secret']}&issuer={issuer}",
    "session_token_prefix": login["session_token"][:24],
    "register_code_delivery": register.get("code_delivery"),
}
print(json.dumps(summary, ensure_ascii=False))
PY
