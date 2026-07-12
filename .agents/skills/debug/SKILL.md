---
name: debug
description: Systematic root-cause debugging — reproduce, isolate, fix at the source, prove the fix. Use when the user reports a bug, an error, a crash, flaky behavior, "this doesn't work", "why is this failing", or pastes a stack trace or failing test output.
---

# Debug

Find root cause before touching code. Fix without understood cause = guess wearing fix's clothes.

## Step 1 — Reproduce

- Deterministic repro first: failing test, `--demo --dump` diff, exact key sequence in TUI. Can't trigger = can't prove fixed.
- Read actual error verbatim — full message, full backtrace (`RUST_BACKTRACE=1`). Answer often in the line the eye skips.
- Capture repro as failing `#[test]` now when feasible — becomes regression test in Step 5 for free.
- Flaky: loop it (`while cargo test bad_test; do :; done` or `-- --test-threads=1` to expose ordering) — establish failure rate before changing anything.

## Step 2 — Shrink search space

Cheapest checks first:

1. **Recent changes**: `git log --oneline -15`, `git diff HEAD~5` on touched area. Most bugs days old, not years. `git bisect` when known-good commit exists and repro is scriptable.
2. **Boundaries**: bad value born here or delivered here? Probe at boundary (sysfs read in, nusb descriptor in, tree flatten out) to decide which side owns bug.
3. **Binary search**: instrument midpoint, pick wrong half, repeat. No shotgunning ten prints — each probe answers one question.
4. **One assumption per probe**: "this sysfs path exists", "this branch runs", "usb.ids actually loaded". Verify, don't assume.

## Step 3 — Root cause

- Ask "why" past first plausible answer. Symptom: panic on unwrap. Cause: field `None`. Root cause: scan skips devices without that descriptor. Fix the scan.
- Fixing shared fn? Grep callers first. Bug can bite them too → fix belongs in shared path, not the one caller from the report.
- State root cause in one sentence. Can't = haven't found it — back to Step 2.

## Step 4 — Fix

- Smallest change removing root cause. No drive-by refactors (see `refactor` skill).
- Real fix large + something bleeding? Guard + TODO acceptable — say it's mitigation, name real fix, don't call bug closed.

## Step 5 — Prove it

- Step 1 repro now passes. `cargo test` green. `cargo clippy -- -D warnings` clean.
- Remove every temp probe/`eprintln!`.
- Report: root cause (one sentence), fix, verification. Tests still fail → say so.

## Rust probes

- `RUST_BACKTRACE=1` (or `=full`) on any panic.
- `cargo test bad_test -- --nocapture` — isolate one test, see prints.
- `dbg!(expr)` over `println!` — shows file:line + expression, greppable to remove.
- TUI eats stdout: probe with `eprintln!` + `2>/tmp/probe.log`, or drop to `--dump` path where prints are visible.
- No hardware / weird topology: reproduce in `--demo`, or fake sysfs dir if bug is linux-path parsing.
