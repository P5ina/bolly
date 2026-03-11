#!/usr/bin/env bash
set -euo pipefail

# Migrate existing tenant machines to plan-based memory sizes.
# Reads plan from the landing DB and updates each Fly machine's guest config.
#
# Usage: ./scripts/migrate-memory.sh
# Requires: FLY_API_TOKEN, DATABASE_URL env vars

API="https://api.machines.dev/v1"

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

# Plan -> memory mapping (must match landing/src/lib/server/stripe/index.ts)
plan_memory() {
  case "$1" in
    starter)   echo 512  ;;
    companion) echo 1024 ;;
    unlimited) echo 2048 ;;
    *)         echo 512  ;;
  esac
}

# Fetch all running tenants with their plan and fly IDs
echo "Fetching tenants from database..."
tenants=$(psql "$DATABASE_URL" -t -A -F '|' -c \
  "SELECT slug, plan, fly_app_id, fly_machine_id FROM tenants WHERE status = 'running' AND fly_app_id IS NOT NULL AND fly_machine_id IS NOT NULL")

if [ -z "$tenants" ]; then
  echo "No running tenants found."
  exit 0
fi

count=$(echo "$tenants" | wc -l | tr -d ' ')
echo "Found $count running tenant(s)."
echo ""

failed=0
updated=0
skipped=0

while IFS='|' read -r slug plan app_id machine_id; do
  target_mb=$(plan_memory "$plan")
  echo -n "  [$slug] plan=$plan target=${target_mb}MB ... "

  # Get current machine config
  config=$(curl -sf -H "$(auth_header)" "$API/apps/$app_id/machines/$machine_id" 2>/dev/null) || {
    echo "FAILED (could not fetch machine)"
    failed=$((failed + 1))
    continue
  }

  current_mb=$(echo "$config" | jq -r '.config.guest.memory_mb // 0')

  if [ "$current_mb" -ge "$target_mb" ]; then
    echo "already ${current_mb}MB, skipped"
    skipped=$((skipped + 1))
    continue
  fi

  # Update guest config with new memory
  update_payload=$(echo "$config" | jq --argjson mem "$target_mb" '{
    config: (.config | .guest.memory_mb = $mem)
  }')

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

  echo "${current_mb}MB -> ${target_mb}MB done"
  updated=$((updated + 1))
done <<< "$tenants"

echo ""
echo "Results: $updated updated, $skipped skipped, $failed failed"
[ "$failed" -gt 0 ] && exit 1
exit 0
