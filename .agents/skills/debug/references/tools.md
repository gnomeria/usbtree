# Per-stack debugging tools

## Go

- `go test -run 'TestName' -v ./pkg/...` — isolate one test. `-count=100` for flakes, `-race` for any concurrency suspicion (goroutine leaks, map races). Race detector first, staring at code second.
- Delve: `dlv test ./pkg -- -test.run TestName`, breakpoints with `b file.go:42`, `p variable`. For servers: `dlv debug ./cmd/app`.
- `slog` probes: prefer a temporary `slog.Debug` with key-values over `fmt.Println` — greppable, structured, obvious to remove.
- Goroutine dumps: send `SIGQUIT` (Ctrl-\) to a stuck process for a full stack dump; `pprof` (`import _ "net/http/pprof"`) for leaks — check `/debug/pprof/goroutine?debug=2`.
- Panics in HTTP handlers: check recovery middleware isn't swallowing the stack trace.

## Node/TS

- Isolate one test: `vitest run path/to.test.ts -t 'name'`. Flakes: `--retry=0 --sequence.shuffle` to expose order-dependence.
- Debugger: `node --inspect-brk` + chrome://inspect, or VS Code JS debug terminal. For tsx: `tsx --inspect src/main.ts`.
- Unhandled rejections silently dropped: run with `--unhandled-rejections=strict` when a promise chain "does nothing".
- `console.dir(obj, { depth: null })` — never trust the default 2-level print for nested state.
- Async stack traces: ensure sourcemaps on (`"sourceMap": true`) or traces point at compiled lines.

## Frontend (React/Svelte/Solid/Astro)

- Hydration mismatch: diff server HTML (view-source) vs client render; usual suspects — locale/time rendering, `Math.random`, browser-only APIs during SSR.
- React: StrictMode double-invoke exposes effect impurity; React DevTools "highlight updates" for re-render storms; `use client` boundary errors → check what the client component transitively imports.
- Svelte 5: `$inspect(value)` traces rune updates; effect loops → check `$effect` writing to state it reads.
- Solid: components run once — a "stale" value is usually destructured props or a signal read outside a tracking scope.
- Network: devtools Network tab before code — wrong request beats wrong render 9 times out of 10.

## SQL/data bugs

- Log the actual query + bound params (pgx tracer, drizzle logger, prisma `log: ['query']`) — ORMs generate surprises.
- Run the suspicious query directly against the dev DB with `EXPLAIN ANALYZE` if it's slow, or with hand-substituted params if it's wrong.
- Timezone bugs: check column type (`timestamp` vs `timestamptz`) before checking code.

## When stuck

- Rubber-duck the data flow out loud in one paragraph; the contradiction usually surfaces.
- `git stash` your probes and re-read the original error with fresh eyes — the answer is disproportionately often in the first error line, not the tenth probe.
- Reduce to a 20-line standalone repro; half the time the act of reduction reveals the bug.
