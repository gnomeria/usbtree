---
id: decision-upgrade-reuses-installer
type: decision
anchors: [src/cli.rs, scripts/install.sh, scripts/install.ps1]
---
# `--upgrade` shells out to the installer, no self-update logic in Rust

`usbtree --upgrade` runs `scripts/install.sh` (`install.ps1` on Windows) fetched
from `main`, same as the curl|sh path — installer stays the single source of
truth for asset names, checksum verify, install dir, macOS quarantine, sudo
symlink. Env vars (`USBTREE_VERSION`, `USBTREE_INSTALL_DIR`, …) are inherited,
so pinning/downgrading works for free. Rejected: self-replacing binary (needs
release-asset + sha logic duplicated in Rust, and Windows can't overwrite a
running exe). Cost: needs network + curl/wget; Homebrew installs should use
`brew upgrade` instead. Installer compares the resolved release to the binary
at its target path: equal versions exit successfully without downloading;
different versions print the transition before replacement. The shell one-liner
downloads to a temp file before running it — a failed fetch piped into `sh` is
an empty script that exits 0.

## Relations
- part_of: module-main
