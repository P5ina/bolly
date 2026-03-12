#!/usr/bin/env bash
set -euo pipefail

# Migrate all tenant machines from Anthropic to OpenRouter.
# Sets OPENROUTER_API_KEY and removes ANTHROPIC_API_KEY env var.
# Usage: OPENROUTER_API_KEY="..." ./scripts/migrate-to-openrouter.sh
# Requires: FLY_API_TOKEN, OPENROUTER_API_KEY env vars

API="https://api.machines.dev/v1"

if [ -z "${FLY_API_TOKEN:-}" ]; then
  echo "Error: FLY_API_TOKEN not set"
  exit 1
fi

if [ -z "${OPENROUTER_API_KEY:-}" ]; then
  echo "Error: OPENROUTER_API_KEY not set"
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

echo "Fetching tenant apps..."
apps=$(curl -sf -H "$(auth_header)" "$API/apps?org_slug=personal" \
  | jq -r '.apps[]?.name // empty' \
  | grep '^bolly-' \
  | grep -v '^bolly$' || true)

if [ -z "$apps" ]; then
  echo "No tenant apps found."
  exit 0
fi

count=$(echo "$apps" | wc -l | tr -d ' ')
echo "Found $count tenant app(s). Migrating to OpenRouter..."
echo ""

failed=0
skipped=0
for app in $apps; do
  machines=$(curl -sf -H "$(auth_header)" "$API/apps/$app/machines" \
    | jq -r '.[]?.id // empty' || true)

  if [ -z "$machines" ]; then
    echo "  [$app] no machines, skipping"
    continue
  fi

  for machine_id in $machines; do
    echo -n "  [$app] $machine_id"

    config=$(curl -sf -H "$(auth_header)" "$API/apps/$app/machines/$machine_id")

    # Check if already on OpenRouter
    existing_or_key=$(echo "$config" | jq -r '.config.env.OPENROUTER_API_KEY // empty')
    if [ -n "$existing_or_key" ]; then
      echo " already on OpenRouter, skipping"
      skipped=$((skipped + 1))
      continue
    fi

    echo -n " migrating..."

    # Set OPENROUTER_API_KEY, remove ANTHROPIC_API_KEY
    update_payload=$(echo "$config" | jq --arg key "$OPENROUTER_API_KEY" '{
      config: (.config | .env.OPENROUTER_API_KEY = $key | del(.env.ANTHROPIC_API_KEY))
    }')

    result=$(curl -sf -X POST \
      -H "$(auth_header)" \
      -H "Content-Type: application/json" \
      -d "$update_payload" \
      "$API/apps/$app/machines/$machine_id" 2>&1) || {
      echo " FAILED"
      echo "    Error: $result"
      failed=$((failed + 1))
      continue
    }

    echo " done"
  done
done

echo ""
echo "Migration complete. Skipped: $skipped (already migrated)."
if [ "$failed" -gt 0 ]; then
  echo "Failed: $failed machine(s)."
  exit 1
else
  echo "All machines migrated successfully."
fi
