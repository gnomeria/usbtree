---
name: write-tests
description: Author tests that match the repo's stack and existing test style, at the cheapest level that catches the regression. Use when the user says "write tests for", "add test coverage", "test this function/handler/component", "this needs tests", or after implementing a feature that lacks tests.
---

# Write Tests

Write tests that fail when the behavior breaks and pass when it doesn't — nothing more. Always run them at the end and report results honestly.

## Step 1: Detect stack and existing setup

- `go.mod` → Go stdlib `testing`. Check whether the repo uses `testify` or plain assertions and match it.
- `package.json` → look for `vitest` (default assumption, v2+), `jest`, `@testing-library/react` / `@testing-library/svelte` / `@solidjs/testing-library`, `@playwright/test`. Check `vitest.config.*` / `vite.config.*` for environment (`node` vs `jsdom`/`happy-dom`) and setup files.
- Find 2–3 existing test files near the code under test and read them. Mirror their location convention (`foo_test.go` beside source; `foo.test.ts` beside source vs `__tests__/` vs `tests/`), naming, imports, and helper usage. If the repo has test utilities (factories, fixtures, a test app builder), use them — do not build parallel ones.

If there is no test setup at all, install the minimal standard for the stack (Go needs nothing; TS: `vitest` + config matching the framework's docs) and say so in your report.

## Step 2: Choose the level

Pick the **cheapest level that would catch the regression you care about**:

- **Unit** — pure logic, parsing, calculations, branching. Default choice. No I/O, no framework.
- **Integration** — handler + routing + validation together, DB queries against a real (local/test) database, service with its real store. Choose this when the risk is in the wiring, not the logic.
- **E2E** — full user flows through a browser. Only for critical paths (signup, checkout) that unit/integration can't verify. Defer to the repo's e2e skill if it has one, else e2e-playwright from this catalog; don't write ad-hoc Playwright here.

Testing a pure function through a browser test, or a routing bug with a unit test that bypasses the router, are both wrong — match the level to where the bug would live.

## Step 3: Write the tests

### Rules (all stacks)

- **Test behavior, not implementation.** Assert on outputs, state changes, and responses — not on "method X was called with Y" unless the call *is* the contract (e.g., "sends an email").
- **Name tests by behavior**: `TestParseAmount_RejectsNegative`, `it("returns 404 when the user does not exist")` — not `test1` or `testHandleUser`.
- **Each test independent**: own setup, own data, no reliance on execution order or state left by another test. Parallel-safe where the framework supports it.
- **No snapshot tests for logic.** Snapshots are acceptable only for genuinely serialized output (a generated config file); for behavior, assert specific fields.
- Cover the happy path, the interesting failure paths (validation, not-found, permission), and boundary values. Skip permutations that exercise the same branch.
- Don't test the framework or the library — test *your* code's use of it.

### Go

Table-driven tests with named cases and `t.Run` subtests — follow the go-service skill's conventions (test struct, `t.Fatalf` for preconditions, `t.Errorf` for assertions, `t.Parallel()` where safe). For handlers, use `httptest.NewRequest` + `httptest.NewRecorder` against the handler or router. Hand-write small fakes against consumer-side interfaces; no mock frameworks. Use `t.Cleanup` for teardown and `t.TempDir` for filesystem tests.

### TypeScript (vitest)

`describe` per unit, `it` per behavior. Prefer **dependency injection over `vi.mock`**: if a function takes its collaborators as arguments, pass fakes; reach for `vi.mock` only for module boundaries you can't inject (and reset with `vi.restoreAllMocks` in `afterEach`). Use `vi.useFakeTimers()` for time-dependent code, `expect(...).rejects.toThrow` for async failures. Async tests must `await` — a floating promise makes the test pass vacuously.

### React / Solid components (testing-library)

Render, interact via `userEvent`, assert on what the user sees:

- Query by role/label/text (`getByRole('button', { name: /save/i })`), never by class name or test-id unless there's no accessible handle.
- `await user.click(...)` then assert with `findBy*`/`waitFor` for async updates.
- Test the component's contract: given props/state, the right thing renders; on interaction, the right callback fires or the right UI appears. Do not assert on internal state or hook internals.
- Solid: same API via `@solidjs/testing-library`; remember effects are synchronous but resource-driven UI still needs `findBy*`.

### E2E

Pointer only: identify the 1–3 critical flows worth E2E and defer to the repo's e2e skill (or e2e-playwright from this catalog) for authoring (Playwright 1.4x, web-first assertions, no fixed sleeps).

## Step 4: Run and report

Run exactly what you wrote first (`go test ./pkg/... -run TestX -v`; `bunx vitest run path/to/file.test.ts` — match the repo's package runner: bunx/pnpm/npx), then the broader suite if it's cheap. Then report:

- What passed, what failed, and *why* — verbatim failure output for anything red.
- If a test fails because it caught a real bug in the code under test, say so and fix the code (or flag it), not the test.
- Never weaken an assertion, add a skip, or loosen a matcher just to get green. Never report green without having run the tests.
