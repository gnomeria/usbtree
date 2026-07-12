---
name: ci-github-actions
description: GitHub Actions CI pipeline conventions — job shape, caching, permissions, and action pinning. Use when asked to "set up CI", "add a GitHub Actions workflow", "fix the pipeline", or when creating/reviewing files under .github/workflows/ for Go or Node/TS projects.
---

# CI with GitHub Actions

Complete copy-paste workflows for Go and Node/TS live in `references/workflows.md` —
read it when writing an actual workflow file. This file is the rules.

## Pipeline shape

Default pipeline: **lint → typecheck → test → build**, as parallel jobs (they don't
depend on each other's outputs) with `build` optionally `needs:` the rest if you want
fail-fast economics. Trigger:

```yaml
on:
  push:
    branches: [main]
  pull_request:
```

Don't run `push` on every branch *and* `pull_request` — that double-builds PR branches.

## Concurrency — cancel superseded runs

Every CI workflow gets this; without it, ten pushes to a PR queue ten full runs:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
```

For deploy workflows, keep the group but set `cancel-in-progress: false` (queue, don't kill mid-deploy).

## Permissions — least privilege

Top of every workflow:

```yaml
permissions:
  contents: read
```

Grant more only per-job, only what that job proves it needs (`pull-requests: write` for a
comment bot, `id-token: write` for OIDC cloud auth, `packages: write` for pushing images).
Never leave the org/repo default write-all doing the work implicitly.

## Pin third-party actions to SHA

- Official `actions/*` and other heavily-trusted orgs: major tag is acceptable (`actions/checkout@v5`).
- **Everything else: pin to a full commit SHA** with a version comment:

  ```yaml
  uses: some-org/some-action@3d1a2b... # v2.1.0
  ```

  Tags are mutable; a compromised action re-tagged at `v2` runs in your CI with your
  secrets. Dependabot/Renovate can bump SHAs for you — enable it.

## Caching

- **Go**: `actions/setup-go@v6` with `cache: true` (default when a `go.sum` exists) — caches module and build cache. Don't hand-roll `actions/cache` for Go.
- **Node/npm or yarn**: `actions/setup-node@v5` with `cache: npm` — caches the package cache (correct; don't cache `node_modules`).
- **pnpm**: setup-node's `cache: pnpm` caches the pnpm store. Install pnpm first (`pnpm/action-setup`, SHA-pinned), then setup-node with `cache: pnpm`, then `pnpm install --frozen-lockfile`.
- **bun**: `oven-sh/setup-bun` (SHA-pinned) has no built-in caching — pair with `actions/cache` on `~/.bun/install/cache`, key `bun-${{ runner.os }}-${{ runner.arch }}-${{ hashFiles('**/bun.lock') }}`, then `bun install --frozen-lockfile`.
- Cache keys must include the lockfile hash — built-in caching does this for you.
- Never cache build outputs between CI runs as a correctness shortcut; only as a measured optimization (Next.js `.next/cache` is the common legitimate case).

## Matrix — only when genuinely needed

A matrix multiplies CI cost. Use one when you actually support multiple targets
(a library testing Node 20/22, or linux+windows CLIs). An app deployed to one runtime
tests on that one runtime version — pin it to the version production runs.

## Secrets

- Secrets come from GitHub **Environments** (`environment: production` on the job), which gate them behind protection rules and reviewers — not from repo-wide secrets sprayed into every job.
- CI jobs (lint/test/build) should need zero secrets. If a test needs a secret, question the test.
- Never `echo` secrets, never pass them as CLI args (visible in process lists/logs). Prefer OIDC (`id-token: write` + cloud role assumption) over long-lived cloud keys.
- `pull_request_target` + checkout of PR head = classic secret-exfiltration hole. Don't use `pull_request_target` unless you know exactly why.

## Monorepo path filters

Scope workflows to what changed:

```yaml
on:
  pull_request:
    paths:
      - "services/api/**"
      - ".github/workflows/api.yml"
```

Include the workflow file itself in its own paths. If a filtered check is a required
status check, add a no-op fallback workflow with the inverse `paths-ignore` so PRs that
don't touch the service aren't blocked forever.

## Artifacts

Upload build outputs when a later job or a human needs them:

```yaml
- uses: actions/upload-artifact@v5
  with:
    name: server-dist
    path: dist/
    retention-days: 7
```

Set `retention-days` deliberately (default 90 wastes storage). Pass files between jobs
via artifacts, not by rebuilding. Test reports/coverage: upload with
`if: always()` so failures still produce the report.

## Checklist for a new workflow

- [ ] Triggers: `push` to main + `pull_request`, no double-trigger
- [ ] `concurrency` with `cancel-in-progress: true`
- [ ] `permissions: contents: read` at top; extras per-job only
- [ ] Third-party actions SHA-pinned with version comment
- [ ] setup-go / setup-node built-in caching, keyed off lockfile
- [ ] Installs use lockfile-strict mode (`npm ci`, `pnpm install --frozen-lockfile`, `bun install --frozen-lockfile`)
- [ ] No matrix unless multiple targets are truly supported
- [ ] Secrets via environments; none in lint/test/build jobs
- [ ] Monorepo: `paths` filters incl. the workflow file
- [ ] Artifacts have `retention-days`; reports uploaded `if: always()`
- [ ] Runtime versions read from the repo (`go.mod`, `.nvmrc`/`package.json engines`), not hardcoded twice
