# Project conventions

## Versioning

Single source of truth: `VERSION` file in repo root.

To bump version across all packages (server, client, landing, desktop):
```sh
./scripts/bump-version.sh 0.14.0
```

Or edit `VERSION` and run without args to sync:
```sh
./scripts/bump-version.sh
```

After bumping, commit all changed files, tag, and push:
```sh
git add -A
git commit -m "Bump version to vX.Y.Z"
git tag vX.Y.Z
git push && git push origin vX.Y.Z
```

## Package manager

Use `pnpm` (not npm) for client and landing.

## Migrations

Never write migrations manually — always use `drizzle-kit generate`.
