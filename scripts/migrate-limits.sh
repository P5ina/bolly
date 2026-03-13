#!/usr/bin/env bash
set -euo pipefail

# Migrate all existing tenants to new token/message limits.
# Uses Node + @neondatabase/serverless (same as other migration scripts).
#
# New limits:
#   starter:   150 msgs/day,  1M tokens/month   (was 100/500K)
#   companion: 500 msgs/day,  3M tokens/month   (was 300/1M)
#   unlimited: -1 msgs/day,  10M tokens/month   (was -1/5M)
#
# Usage: DATABASE_URL="postgres://..." ./scripts/migrate-limits.sh
# Requires: DATABASE_URL env var, node, @neondatabase/serverless

if [ -z "${DATABASE_URL:-}" ]; then
  echo "Error: DATABASE_URL not set"
  exit 1
fi

# Run from landing/ so @neondatabase/serverless is resolvable
cd "$(dirname "$0")/../landing"

echo "Current limits:"
node -e "
  const { neon } = require('@neondatabase/serverless');
  const sql = neon(process.env.DATABASE_URL);
  sql\`SELECT plan, count(*)::int as tenants, messages_per_day, tokens_per_month
      FROM tenants WHERE status != 'destroyed'
      GROUP BY plan, messages_per_day, tokens_per_month
      ORDER BY plan\`
    .then(rows => {
      console.log('  plan        | tenants | msgs/day | tokens/month');
      console.log('  ------------|---------|----------|-------------');
      rows.forEach(r => console.log('  ' +
        r.plan.padEnd(12) + '| ' +
        String(r.tenants).padEnd(8) + '| ' +
        String(r.messages_per_day).padEnd(9) + '| ' +
        r.tokens_per_month
      ));
    })
    .catch(e => { console.error(e.message); process.exit(1); });
" || exit 1

echo ""
echo "Updating starter: 150 msgs/day, 1M tokens/month..."
node -e "
  const { neon } = require('@neondatabase/serverless');
  const sql = neon(process.env.DATABASE_URL);
  sql\`UPDATE tenants
      SET messages_per_day = 150, tokens_per_month = 1000000, updated_at = NOW()
      WHERE plan = 'starter' AND status != 'destroyed'\`
    .then(() => console.log('  done'))
    .catch(e => { console.error(e.message); process.exit(1); });
" || exit 1

echo "Updating companion: 500 msgs/day, 3M tokens/month..."
node -e "
  const { neon } = require('@neondatabase/serverless');
  const sql = neon(process.env.DATABASE_URL);
  sql\`UPDATE tenants
      SET messages_per_day = 500, tokens_per_month = 3000000, updated_at = NOW()
      WHERE plan = 'companion' AND status != 'destroyed'\`
    .then(() => console.log('  done'))
    .catch(e => { console.error(e.message); process.exit(1); });
" || exit 1

echo "Updating unlimited: unlimited msgs/day, 10M tokens/month..."
node -e "
  const { neon } = require('@neondatabase/serverless');
  const sql = neon(process.env.DATABASE_URL);
  sql\`UPDATE tenants
      SET messages_per_day = -1, tokens_per_month = 10000000, updated_at = NOW()
      WHERE plan = 'unlimited' AND status != 'destroyed'\`
    .then(() => console.log('  done'))
    .catch(e => { console.error(e.message); process.exit(1); });
" || exit 1

echo ""
echo "New limits:"
node -e "
  const { neon } = require('@neondatabase/serverless');
  const sql = neon(process.env.DATABASE_URL);
  sql\`SELECT plan, count(*)::int as tenants, messages_per_day, tokens_per_month
      FROM tenants WHERE status != 'destroyed'
      GROUP BY plan, messages_per_day, tokens_per_month
      ORDER BY plan\`
    .then(rows => {
      console.log('  plan        | tenants | msgs/day | tokens/month');
      console.log('  ------------|---------|----------|-------------');
      rows.forEach(r => console.log('  ' +
        r.plan.padEnd(12) + '| ' +
        String(r.tenants).padEnd(8) + '| ' +
        String(r.messages_per_day).padEnd(9) + '| ' +
        r.tokens_per_month
      ));
    })
    .catch(e => { console.error(e.message); process.exit(1); });
" || exit 1

echo ""
echo "Done."
