---
name: debug
description: Systematic root-cause debugging — reproduce, isolate, fix at the source, prove the fix. Use when the user reports a bug, an error, a crash, flaky behavior, "this doesn't work", "why is this failing", or pastes a stack trace or failing test output.
---

# Debug

Find the root cause before touching code. A fix without a understood cause is a guess wearing a fix's clothes.

## Step 1 — Reproduce

- Get a deterministic reproduction before anything else: failing test, curl command, or exact click path. If you can't trigger it, you can't prove you fixed it.
- Read the actual error verbatim — full message, full stack trace, not a paraphrase. The answer is often in the line the eye skips.
- Capture the reproduction as a failing automated test *now* when feasible. It becomes the regression test in Step 5 for free.
- Flaky bugs: run the repro in a loop (`go test -run X -count=100`, `vitest --retry=0` repeated) to establish a failure rate before changing anything.

## Step 2 — Shrink the search space

Cheapest checks first:

1. **Recent changes**: `git log --oneline -15` and `git diff HEAD~5` on the touched area. Most bugs are days old, not years. `git bisect` when a known-good commit exists and the repro is scriptable.
2. **Boundaries**: is the bad value born here or delivered here? Log/inspect at the boundary (request in, DB out, API response) to decide which side owns the bug.
3. **Binary search**: cut the path in half — instrument the midpoint, determine which half is wrong, repeat. Never shotgun ten print statements at once; each probe should answer one question.
4. **Question one assumption per probe**: "this config is loaded", "this branch runs", "this is the version deployed". Verify, don't assume — the bug lives in a false assumption.

## Step 3 — Identify root cause

- Keep asking "why" past the first plausible answer. Symptom: nil deref. Cause: field unset. Root cause: constructor allows partial init. Fix the constructor.
- Before fixing a shared function, grep its callers. If the bug can bite them too, the fix belongs in the shared code path, not in the one caller from the bug report.
- State the root cause in one sentence. If you can't, you haven't found it — return to Step 2.

## Step 4 — Fix

- Smallest change that removes the root cause. No drive-by refactors, no "while I'm here" — separate commits for that (see `refactor` skill).
- If the real fix is large and something is bleeding in prod, a guard + TODO is acceptable — but say explicitly it's a mitigation, name the real fix, and don't call the bug closed.

## Step 5 — Prove it

- The Step 1 reproduction now passes. The rest of the test suite still passes.
- Remove every temporary probe/print you added.
- Report: root cause (one sentence), the fix, how it's verified. If tests still fail, say so — never declare victory on partial evidence.

## Per-stack probes

Read `references/tools.md` when you need stack-specific instrumentation: delve / `go test -race`, Node `--inspect` and vitest filtering, browser devtools for frontend state/hydration issues, SQL logging for query bugs.
