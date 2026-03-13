#!/usr/bin/env bash
set -euo pipefail

# Migrate existing tenant machines to add BOLLY_PUBLIC_URL env var.
# Each tenant gets https://{slug}.{BOLLY_DOMAIN} as its public URL.
#
# Usage: ./scripts/migrate-public-url.sh
# Requires: FLY_API_TOKEN, DATABASE_URL env vars
# Optional: BOLLY_DOMAIN (default: bollyai.dev)

API="https://api.machines.dev/v1"
DOMAIN="${BOLLY_DOMAIN:-bollyai.dev}"

if [ -z "${FLY_API_TOKEN:-}" ]; then
  echo "Error: FLY_API_TOKEN not set"
  exit 1
fi

if [ -z "${DATABASE_URL:-}" ]; then
  echo "Error: DATABASE_URL not set"
  exit 1
fi

auth_header() {
  local token="$FLY_API_TOKEN"
  if [[ "$token" == FlyV1* ]]; then
    echo "Authorization: $token"
  else
    echo "Authorization: Bearer $token"
  fi
}

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LANDING_DIR="$SCRIPT_DIR/../landing"

echo "Fetching tenants from database..."
tenants=$(cd "$LANDING_DIR" && node -e "
  const { neon } = require('@neondatabase/serverless');
  const sql = neon(process.env.DATABASE_URL);
  sql\`SELECT slug, fly_app_id, fly_machine_id FROM tenants
      WHERE status = 'running' AND fly_app_id IS NOT NULL AND fly_machine_id IS NOT NULL\`
    .then(rows => rows.forEach(r => console.log([r.slug, r.fly_app_id, r.fly_machine_id].join('|'))))
    .catch(e => { console.error(e.message); process.exit(1); });
" 2>&1) || { echo "DB query failed: $tenants"; exit 1; }

if [ -z "$tenants" ]; then
  echo "No running tenants found."
  exit 0
fi

count=$(echo "$tenants" | wc -l | tr -d ' ')
echo "Found $count running tenant(s). Domain: $DOMAIN"
echo ""

failed=0
updated=0

while IFS='|' read -r slug app_id machine_id; do
  public_url="https://${slug}.${DOMAIN}"
  echo -n "  [$slug] $public_url ... "

  config=$(curl -sf -H "$(auth_header)" "$API/apps/$app_id/machines/$machine_id" 2>/dev/null) || {
    echo "FAILED (could not fetch machine)"
    failed=$((failed + 1))
    continue
  }

  update_payload=$(echo "$config" | jq --arg url "$public_url" '{ config: (.config | .env.BOLLY_PUBLIC_URL = $url) }')

  result=$(curl -sf -X POST \
    -H "$(auth_header)" \
    -H "Content-Type: application/json" \
    -d "$update_payload" \
    "$API/apps/$app_id/machines/$machine_id" 2>&1) || {
    echo "FAILED"
    echo "    Error: $result"
    failed=$((failed + 1))
    continue
  }

  echo "done"
  updated=$((updated + 1))
done <<< "$tenants"

echo ""
echo "Results: $updated updated, $failed failed"
[ "$failed" -gt 0 ] && exit 1
exit 0
