#!/usr/bin/env bash
set -euo pipefail

# Migrate existing tenant volumes to new plan-based storage sizes.
# Fly volumes can only be extended, not shrunk — this is safe.
#
# Usage: ./scripts/migrate-storage.sh
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

# Plan -> storage GB mapping (must match landing/src/lib/server/stripe/index.ts)
plan_storage_gb() {
  case "$1" in
    starter)   echo 10 ;;
    companion) echo 20 ;;
    unlimited) echo 50 ;;
    *)         echo 10 ;;
  esac
}

# Fetch all running tenants with their plan and fly IDs
echo "Fetching tenants from database..."
tenants=$(node -e "
  const { neon } = require('@neondatabase/serverless');
  const sql = neon(process.env.DATABASE_URL);
  sql\`SELECT slug, plan, fly_app_id, fly_volume_id FROM tenants
      WHERE status = 'running' AND fly_app_id IS NOT NULL AND fly_volume_id IS NOT NULL\`
    .then(rows => rows.forEach(r => console.log([r.slug, r.plan, r.fly_app_id, r.fly_volume_id].join('|'))))
    .catch(e => { console.error(e.message); process.exit(1); });
" 2>&1) || { echo "DB query failed: $tenants"; exit 1; }

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

while IFS='|' read -r slug plan app_id volume_id; do
  target_gb=$(plan_storage_gb "$plan")
  echo -n "  [$slug] plan=$plan target=${target_gb}GB ... "

  # Get current volume info
  vol_info=$(curl -sf -H "$(auth_header)" "$API/apps/$app_id/volumes/$volume_id" 2>/dev/null) || {
    echo "FAILED (could not fetch volume)"
    failed=$((failed + 1))
    continue
  }

  current_gb=$(echo "$vol_info" | jq -r '.size_gb // 0')

  if [ "$current_gb" -ge "$target_gb" ]; then
    echo "already ${current_gb}GB, skipped"
    skipped=$((skipped + 1))
    continue
  fi

  # Extend volume (Fly only supports extending, not shrinking)
  result=$(curl -sf -X PUT \
    -H "$(auth_header)" \
    -H "Content-Type: application/json" \
    -d "{\"size_gb\": $target_gb}" \
    "$API/apps/$app_id/volumes/$volume_id/extend" 2>&1) || {
    echo "FAILED"
    echo "    Error: $result"
    failed=$((failed + 1))
    continue
  }

  echo "${current_gb}GB -> ${target_gb}GB done"
  updated=$((updated + 1))
done <<< "$tenants"

echo ""
echo "Results: $updated updated, $skipped skipped, $failed failed"
[ "$failed" -gt 0 ] && exit 1
exit 0
