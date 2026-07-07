---
name: refactor
description: Behavior-preserving restructuring done safely — test net first, small verified steps, no mixed-in feature changes. Use when the user says "refactor", "clean up", "restructure", "extract", "rename", "split this file/function", "reduce duplication", or "make this testable".
---

# Refactor

Refactoring changes structure, never behavior. If behavior changes, it's a feature or a bug fix — do that separately.

## Step 0 — Decide it's worth it

- Refactor with a purpose: enabling a concrete upcoming change, killing duplication that actually bit someone, or making untestable code testable. "Could be cleaner" alone is not a purpose — say so and stop.
- Scope it: name the target shape in one or two sentences before touching code. No target = wandering diff.

## Step 1 — Safety net

- Run the existing tests covering the code; note the passing baseline. Typecheck/lint too (`go vet`, `tsc --noEmit`).
- **Uncovered code**: write characterization tests first — pin down what it *does now* (including odd behavior), not what it should do. A refactor without a net is just editing and hoping.
- Working tree must be clean before starting; the ability to `git checkout .` at any point is the escape hatch.

## Step 2 — Small steps, verify each

- One named refactoring at a time: extract function, inline, rename, move, split module, introduce parameter. Finish it, verify, then the next.
- Verify after every step: tests + typecheck. Green → keep going or commit-point; red → the last step broke it, undo just that step. Never push forward on red planning to "fix at the end".
- Use mechanical tools for mechanical work: `gopls rename`, TS language-server rename, IDE move-symbol. A tool rename can't miss a call site; a regex can.
- Keep steps commit-sized. On long refactors, stop at green checkpoints so the user can commit — a 40-file big-bang diff that "should work" is failure, not progress.

## Step 3 — Hard rules

- **No behavior changes.** Spot a bug mid-refactor? Note it, finish or checkpoint the refactor, fix the bug as a separate change (see `debug` skill). Never fold it in silently.
- **No API breaks without saying so.** Changing an exported/public signature? Grep all call sites first; update every one in the same change, and flag it to the user if the surface is shared beyond this repo.
- **No test rewrites to fit the refactor.** Tests failing after a "behavior-preserving" change means the change wasn't behavior-preserving. The exception is tests coupled to structure (mocks of a now-gone internal); update those minimally and say why.
- **No scope creep.** The target shape from Step 0 is the boundary. New improvement ideas get listed at the end, not done.

## Step 4 — Finish

- Full test suite + typecheck + lint green. Dead code left behind by the restructure gets deleted, not commented out.
- Report: target shape achieved, steps taken, anything discovered but deliberately not done (bugs found, further refactorings), and any API changes.

## Direction

For which structures to prefer per stack, follow the stack guides (`go-service`, `node-backend`, `react-next`, `sveltekit`, `solidjs`, `astro`). Common calls:

- Prefer extracting *functions* over introducing classes/interfaces; add an interface only at a genuine seam (Go: define it where it's consumed).
- Duplication rule of three: two similar blocks may stand; the third occurrence earns the abstraction — with the right shape now visible.
- Don't abstract for testability when injecting a plain function/value would do.
