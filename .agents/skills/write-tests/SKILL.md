---
name: write-tests
description: Author tests that match the repo's stack and existing test style, at the cheapest level that catches the regression. Use when the user says "write tests for", "add test coverage", "test this function/handler/component", "this needs tests", or after implementing a feature that lacks tests.
---

# Write Tests

Test fails when behavior breaks, passes when it doesn't — nothing more. Run at end, report honestly.

## Step 1 — Match repo style

- Tests live in-file: `#[cfg(test)] mod tests` at bottom, `use super::*`.
- Builder helpers for fat structs (see `dev()` in `src/usb.rs` tests) — reuse existing helper, extend it, don't build parallel one.
- Plain `assert_eq!`/`assert!`. No assertion crates, no mock frameworks.
- Raw descriptor bytes: inline arrays with `#[rustfmt::skip]` + comment per chunk (existing pattern).

## Step 2 — Choose level

Cheapest level that catches the regression:

- **Unit `#[test]`** — pure logic: tree flatten/fold, parsing (usb.ids, sysfs values, descriptors), diff. Default. No hardware, no I/O.
- **Smoke** — `./target/release/usbtree --demo --dump`: exercises scan→tree→render path, no hardware. For wiring bugs, not logic bugs.
- **TUI in pty** — `--demo` in pty for interaction (keys, fold, filter). Manual/scripted, last resort — most TUI logic is testable as pure functions on App state; prefer extracting that.

Pure parse bug tested through pty = wrong. Keybinding wiring tested as unit = also wrong. Match level to where bug lives.

## Step 3 — Write

- **Behavior, not implementation.** Assert outputs and state, not internals.
- **Name by behavior**: `max_power_parses_sysfs`, `filter_finds_devices_in_collapsed_hubs` — not `test1`.
- **Independent tests**: own data, no ordering deps. No shared mutable state — tests run parallel by default.
- Filesystem cases: `tempfile::TempDir` if already a dep, else skip fs test and test the parse fn on a `&str` instead — push I/O to edge, test the pure part.
- Cover happy path, interesting failures (malformed input, missing sysfs file, truncated descriptor), boundary values. Skip permutations hitting same branch.
- Don't test nusb/ratatui — test your use of them.

## Step 4 — Run and report

- `cargo test name_fragment` first, then full `cargo test`.
- Report pass/fail, verbatim output for red.
- Test caught real bug in code under test → fix code (or flag), never the test.
- Never weaken assertion or skip to get green. Never report green without running.
