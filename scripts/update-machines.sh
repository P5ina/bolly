#!/usr/bin/env bash
set -euo pipefail

# Update all tenant machines to the latest image.
# Usage: ./scripts/update-machines.sh [image]
# Requires: FLY_API_TOKEN env var

IMAGE="${1:-registry.fly.io/bolly:latest}"
API="https://api.machines.dev/v1"

if [ -z "${FLY_API_TOKEN:-}" ]; then
  echo "Error: FLY_API_TOKEN not set"
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

# List all apps matching bolly-* pattern
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
echo "Found $count tenant app(s). Updating to $IMAGE..."
echo ""

failed=0
for app in $apps; do
  # Get machines in this app
  machines=$(curl -sf -H "$(auth_header)" "$API/apps/$app/machines" \
    | jq -r '.[]?.id // empty' || true)

  if [ -z "$machines" ]; then
    echo "  [$app] no machines, skipping"
    continue
  fi

  for machine_id in $machines; do
    echo -n "  [$app] updating $machine_id..."

    # Get current machine config
    config=$(curl -sf -H "$(auth_header)" "$API/apps/$app/machines/$machine_id")
    current_image=$(echo "$config" | jq -r '.config.image // empty')

    # Skip nightly machines when updating to stable image (and vice versa)
    if [[ "$IMAGE" == *":latest" ]] && [[ "$current_image" == *":nightly" ]]; then
      echo " skipped (nightly channel)"
      continue
    fi
    if [[ "$IMAGE" == *":nightly" ]] && [[ "$current_image" != *":nightly" ]]; then
      echo " skipped (stable channel)"
      continue
    fi

    # Update machine with new image
    update_payload=$(echo "$config" | jq --arg img "$IMAGE" '{
      config: (.config | .image = $img)
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
