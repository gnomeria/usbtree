# Complete Workflow Examples

## Go service

`.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: actions/setup-go@v6
        with:
          go-version-file: go.mod   # single source of truth; enables module+build cache
      - name: golangci-lint
        uses: golangci/golangci-lint-action@1481404843c368bc19ca9406f87d6e0fc97bdcfd # v7.0.0
        with:
          version: v2.1.6
      - name: go vet
        run: go vet ./...
      - name: check gofmt
        run: test -z "$(gofmt -l .)"

  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:17-alpine
        env:
          POSTGRES_USER: app
          POSTGRES_PASSWORD: test
          POSTGRES_DB: app_test
        ports: ["5432:5432"]
        options: >-
          --health-cmd "pg_isready -U app" --health-interval 5s
          --health-timeout 3s --health-retries 10
    env:
      DATABASE_URL: postgres://app:test@localhost:5432/app_test?sslmode=disable
    steps:
      - uses: actions/checkout@v5
      - uses: actions/setup-go@v6
        with:
          go-version-file: go.mod
      - run: go test -race -count=1 -coverprofile=coverage.out ./...
      - uses: actions/upload-artifact@v5
        if: always()
        with:
          name: coverage
          path: coverage.out
          retention-days: 7

  build:
    runs-on: ubuntu-latest
    needs: [lint, test]
    steps:
      - uses: actions/checkout@v5
      - uses: actions/setup-go@v6
        with:
          go-version-file: go.mod
      - run: CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o bin/server ./cmd/server
      - uses: actions/upload-artifact@v5
        with:
          name: server
          path: bin/server
          retention-days: 7
```

Notes:

- `go-version-file: go.mod` keeps the CI Go version in lockstep with the module. setup-go caching is on by default when `go.sum` exists.
- The postgres `services:` container replaces docker-compose in CI; the health options gate the job steps on readiness.
- `-race -count=1`: race detector always on in CI; `-count=1` defeats stale test caching.
- The golangci-lint SHA above is illustrative — resolve the current release SHA when writing the file.

## Node/TS (pnpm)

`.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  checks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: pnpm/action-setup@a7487c7e89a18df4991f7f222e4898a00d66ddda # v4.1.0
      - uses: actions/setup-node@v5
        with:
          node-version-file: package.json   # reads engines.node (or use .nvmrc)
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - run: pnpm lint
      - run: pnpm typecheck        # tsc --noEmit
      - run: pnpm test

  build:
    runs-on: ubuntu-latest
    needs: [checks]
    steps:
      - uses: actions/checkout@v5
      - uses: pnpm/action-setup@a7487c7e89a18df4991f7f222e4898a00d66ddda # v4.1.0
      - uses: actions/setup-node@v5
        with:
          node-version-file: package.json
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - run: pnpm build
      - uses: actions/upload-artifact@v5
        with:
          name: dist
          path: dist/
          retention-days: 7
```

Notes:

- lint/typecheck/test are merged into one `checks` job here because each is seconds long — splitting into three jobs pays 3x setup cost. Split when a step gets slow (tests > ~2 min) or you want independent status checks.
- pnpm/action-setup reads the pnpm version from `packageManager` in package.json — set it there, not in the workflow. SHA is illustrative; resolve the current one.
- npm projects: drop the pnpm step, use `cache: npm` and `npm ci`.
- bun projects: replace pnpm/setup-node with `oven-sh/setup-bun` (SHA-pinned) + `actions/cache` on `~/.bun/install/cache` keyed on `hashFiles('**/bun.lock')`, then `bun install --frozen-lockfile` and `bun run lint/typecheck/test`.
- Next.js: add `.next/cache` caching keyed on lockfile + source hash if build time hurts; upload the standalone output as the artifact.

## Monorepo variant

Per-package workflow with path filters:

```yaml
on:
  pull_request:
    paths:
      - "services/api/**"
      - "packages/shared/**"          # in-repo deps of the service
      - "pnpm-lock.yaml"
      - ".github/workflows/api.yml"

defaults:
  run:
    working-directory: services/api
```

If `api` checks are required status checks, add the no-op twin so unrelated PRs can merge:

```yaml
# .github/workflows/api-noop.yml — same name and job names as api.yml
name: API CI
on:
  pull_request:
    paths-ignore:
      - "services/api/**"
      - "packages/shared/**"
      - "pnpm-lock.yaml"
      - ".github/workflows/api.yml"
jobs:
  checks:
    runs-on: ubuntu-latest
    steps:
      - run: echo "No relevant changes"
```

## Deploy job skeleton (environments + OIDC)

```yaml
  deploy:
    needs: [build]
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    environment: production          # env-scoped secrets + protection rules
    permissions:
      contents: read
      id-token: write                # OIDC — no long-lived cloud keys
    concurrency:
      group: deploy-production
      cancel-in-progress: false      # queue deploys, never kill one mid-flight
    steps:
      - uses: actions/download-artifact@v5
        with:
          name: server
      # cloud auth via OIDC action (SHA-pinned), then deploy
```
