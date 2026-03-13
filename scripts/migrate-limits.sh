#!/usr/bin/env bash
set -euo pipefail

# Migrate all existing tenants to new token/message limits.
# Reads DATABASE_URL from env and updates the tenants table directly.
#
# New limits:
#   starter:   150 msgs/day,  1M tokens/month   (was 100/500K)
#   companion: 500 msgs/day,  3M tokens/month   (was 300/1M)
#   unlimited: -1 msgs/day,  10M tokens/month   (was -1/5M)
#
# Usage: DATABASE_URL="postgres://..." ./scripts/migrate-limits.sh
# Requires: psql

if [ -z "${DATABASE_URL:-}" ]; then
  echo "Error: DATABASE_URL not set"
  exit 1
fi

echo "Current limits:"
psql "$DATABASE_URL" -c "
  SELECT plan,
         count(*) as tenants,
         messages_per_day,
         tokens_per_month
  FROM tenants
  WHERE status != 'destroyed'
  GROUP BY plan, messages_per_day, tokens_per_month
  ORDER BY plan;
"

echo ""
echo "Updating starter: 150 msgs/day, 1M tokens/month..."
psql "$DATABASE_URL" -c "
  UPDATE tenants
  SET messages_per_day = 150,
      tokens_per_month = 1000000,
      updated_at = NOW()
  WHERE plan = 'starter'
    AND status != 'destroyed';
"

echo "Updating companion: 500 msgs/day, 3M tokens/month..."
psql "$DATABASE_URL" -c "
  UPDATE tenants
  SET messages_per_day = 500,
      tokens_per_month = 3000000,
      updated_at = NOW()
  WHERE plan = 'companion'
    AND status != 'destroyed';
"

echo "Updating unlimited: unlimited msgs/day, 10M tokens/month..."
psql "$DATABASE_URL" -c "
  UPDATE tenants
  SET messages_per_day = -1,
      tokens_per_month = 10000000,
      updated_at = NOW()
  WHERE plan = 'unlimited'
    AND status != 'destroyed';
"

echo ""
echo "New limits:"
psql "$DATABASE_URL" -c "
  SELECT plan,
         count(*) as tenants,
         messages_per_day,
         tokens_per_month
  FROM tenants
  WHERE status != 'destroyed'
  GROUP BY plan, messages_per_day, tokens_per_month
  ORDER BY plan;
"

echo "Done."
