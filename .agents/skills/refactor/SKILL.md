---
name: refactor
description: Behavior-preserving restructuring done safely — test net first, small verified steps, no mixed-in feature changes. Use when the user says "refactor", "clean up", "restructure", "extract", "rename", "split this file/function", "reduce duplication", or "make this testable".
---

# Refactor

Structure changes, behavior never. Behavior changes = feature or bug fix — separate change.

## Step 0 — Worth it?

- Purpose required: enables concrete upcoming change, kills duplication that bit someone, or makes untestable testable. "Could be cleaner" alone = say so, stop.
- Name target shape in one or two sentences before touching code. No target = wandering diff.

## Step 1 — Safety net

- Baseline: `cargo test` green, `cargo clippy -- -D warnings` clean. Note it.
- Uncovered code: characterization tests first — pin what it *does now* (odd behavior included). Extra check: `--demo --dump` output before vs after must be byte-identical.
- Working tree clean before starting; `git checkout .` is the escape hatch.

## Step 2 — Small steps, verify each

- One named refactoring at a time: extract fn, inline, rename, move, split module. Finish, verify, next.
- Verify each step: `cargo test` + clippy. Red → undo last step only. Never push forward on red planning to "fix at the end".
- Rename via rust-analyzer / `cargo` tooling where available — tool can't miss call site, regex can.
- Commit-sized steps. Long refactor → stop at green checkpoints. 40-file big-bang diff "should work" = failure.

## Step 3 — Hard rules

- **No behavior changes.** Bug found mid-refactor → note it, checkpoint, fix separately (see `debug` skill).
- **No fmt churn.** Repo bans blind `cargo fmt` — hand-aligned blocks (theme, match tables). Diff only what the refactor touches.
- **Respect `// ponytail:` comments** — known deliberate tradeoffs. Don't "clean up" without reason.
- **No test rewrites to fit refactor.** Tests fail after "behavior-preserving" change → change wasn't behavior-preserving. Exception: tests coupled to now-gone internals; update minimally, say why.
- **No scope creep.** Step 0 shape is boundary. New ideas → list at end, don't do.

## Step 4 — Finish

- Full `cargo test` + clippy green. `--demo --dump` unchanged. Dead code deleted, not commented out.
- Report: shape achieved, steps taken, things found-not-done.

## Direction

- Extract *functions* over introducing traits/structs; trait only at genuine seam (multiple impls exist or test needs one).
- Rule of three: two similar blocks may stand; third occurrence earns abstraction — right shape now visible.
- Don't abstract for testability when passing plain value/fn does it.
