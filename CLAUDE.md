# CLAUDE.md

caveman brief. read once, know repo.

## what

usbtree = rust TUI. show USB device tree live. ratatui draw, nusb scan. no root. no libusb. linux + mac + windows.

## map

- `src/main.rs` — App, event loop, all drawing, theme (charm pastels). 1s rescan tick.
- `src/usb.rs` — scan via nusb, sysfs-style names (`1-1.4`), tree flatten/fold, usb.ids parse, `~/.config/usbtree/overrides.ids`, `demo_scan`.
- `src/metrics.rs` — activity rates. urbnum sysfs (unprivileged, linux), usbmon (root, real bytes/s), demo (synthetic).
- `scripts/install.sh` — curl|sh installer. asset names MUST match release.yml pattern `usbtree_<ver>_<os>-<arch>.tar.gz` + checksums.txt.
- `scripts/shots.sh` — render tapes. needs vhs + ttyd + ffmpeg.
- `tapes/demo.tape` — VHS walkthrough. timings assume demo loop (below). change demo_scan → check tape still lands events.
- `docs/index.html` — website. self-contained, no external requests. GIF `screenshots/demo.gif`, HTML mockup = fallback when GIF missing.
- `Taskfile.yml` — `task -l`. optional sugar, CI no use it.

## flags

`--dump` print once no TUI. `--updatelist` fetch fresh usb.ids to config dir. `--demo` fake tree, fake traffic, no hardware. demo loop 30s: SSD in @6s out @24s, webcam out @14s back @20s.

## rules

- clippy clean or CI angry: `cargo clippy -- -D warnings`. CI runner clippy maybe newer than local — new lints appear, fix them, no allow-spam.
- NO blind `cargo fmt`. code hand-aligned (theme block, match tables). small diffs, match style around you.
- conventional commits. release-please read them, cut release PR. merge release PR → tag → release.yml build binaries.
- `// ponytail:` comment = known tradeoff, on purpose. no "fix" without reason.
- mac/windows binaries unsigned. keep warnings in README + installer + site when touching install docs.
- hooks: `task hooks` once per clone (gitleaks + privacy scan pre-commit).

## workflows

- `ci.yml` — clippy + test + build, 3 OS, every push/PR.
- `release.yml` — semver tag on main → binaries + checksums. guard: tag must be on main.
- `release-please.yml` — version bookkeeping.
- `pages.yml` — docs/ → github pages on main push.
- `screenshots.yml` — src/ or tapes/ change on main → VHS re-render docs/screenshots, auto-commit. GITHUB_TOKEN push no retrigger, no loop.

## test fast

`cargo test` (pure logic: tree, parse, diff). `./target/release/usbtree --demo --dump` = smoke test no hardware. TUI check: run `--demo` in pty.
