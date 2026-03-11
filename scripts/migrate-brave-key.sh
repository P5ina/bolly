#!/usr/bin/env bash
set -euo pipefail

# Add BRAVE_SEARCH_API_KEY env var to all tenant machines.
# Usage: BRAVE_SEARCH_API_KEY="..." ./scripts/migrate-brave-key.sh
# Requires: FLY_API_TOKEN, BRAVE_SEARCH_API_KEY env vars

API="https://api.machines.dev/v1"

if [ -z "${FLY_API_TOKEN:-}" ]; then
  echo "Error: FLY_API_TOKEN not set"
  exit 1
fi

if [ -z "${BRAVE_SEARCH_API_KEY:-}" ]; then
  echo "Error: BRAVE_SEARCH_API_KEY not set"
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
echo "Found $count tenant app(s). Adding BRAVE_SEARCH_API_KEY..."
echo ""

failed=0
for app in $apps; do
  machines=$(curl -sf -H "$(auth_header)" "$API/apps/$app/machines" \
    | jq -r '.[]?.id // empty' || true)

  if [ -z "$machines" ]; then
    echo "  [$app] no machines, skipping"
    continue
  fi

  for machine_id in $machines; do
    echo -n "  [$app] updating $machine_id..."

    config=$(curl -sf -H "$(auth_header)" "$API/apps/$app/machines/$machine_id")

    update_payload=$(echo "$config" | jq --arg key "$BRAVE_SEARCH_API_KEY" '{
      config: (.config | .env.BRAVE_SEARCH_API_KEY = $key)
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
if [ "$failed" -gt 0 ]; then
  echo "Completed with $failed failure(s)."
  exit 1
else
  echo "All machines updated successfully."
fi
