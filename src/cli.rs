use crate::usb;
use std::collections::HashSet;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Every accepted argument. Keep in sync with `help()` and the `main.rs` dispatch.
pub const KNOWN_FLAGS: &[&str] = &[
    "--help",       "-h",
    "--version",
    "--dump",
    "--pci",
    "--updatelist", "--update-list",
    "--upgrade",
    "--demo",
    "--light",
    "--nerd-font",  "--nerd-fonts",
    "--ascii",
];

const INSTALL_SH: &str = "https://raw.githubusercontent.com/gnomeria/usbtree/main/scripts/install.sh";
const INSTALL_PS1: &str =
    "https://raw.githubusercontent.com/gnomeria/usbtree/main/scripts/install.ps1";

/// `--version`
pub fn version() {
    println!("usbtree {VERSION}");
}

/// `--help`
pub fn help() {
    println!(
        "usbtree {VERSION} — live USB device tree in the terminal

Usage: usbtree [OPTIONS]

Options:
  --help, -h    show this help and exit
  --version     show version and exit
  --dump        print the tree once and exit (no TUI)
  --pci         print the PCI list once and exit (no TUI)
  --updatelist  download the latest usb.ids into the config dir
  --upgrade     update usbtree in place via the official installer
  --demo        fake tree with scripted hot-plug + traffic (no hardware)
  --light       light theme (for light-background terminals)
  --nerd-font   nerd-font icons
  --ascii       ascii-only icons"
    );
}

/// `--upgrade` — re-run the official installer, which fetches the latest
/// release for this platform and overwrites the installed binary.
/// Env vars the installer reads (`USBTREE_VERSION`, `USBTREE_INSTALL_DIR`, …)
/// are inherited, so `USBTREE_VERSION=0.1.0 usbtree --upgrade` downgrades.
pub fn upgrade() -> std::io::Result<()> {
    let (prog, args) = if cfg!(windows) {
        (
            "powershell",
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                format!("irm {INSTALL_PS1} | iex"),
            ],
        )
    } else {
        // Download to a temp file instead of piping into `sh`: a failed fetch
        // piped to a shell is an empty script that "succeeds" silently.
        (
            "sh",
            vec![
                "-c".to_string(),
                format!(
                    "set -e; T=$(mktemp); \
                     if command -v curl >/dev/null 2>&1; then curl -fsSL {INSTALL_SH} -o \"$T\"; \
                     else wget -qO \"$T\" {INSTALL_SH}; fi; \
                     sh \"$T\"; rm -f \"$T\""
                ),
            ],
        )
    };

    let src = if cfg!(windows) { INSTALL_PS1 } else { INSTALL_SH };
    println!("usbtree {VERSION} — updating via {src}\n");
    let status = std::process::Command::new(prog).args(&args).status()?;
    if !status.success() {
        eprintln!("upgrade failed ({status})");
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

/// Non-TUI mode: print the tree once and exit.
pub fn dump(demo: bool) {
    let devices = if demo { usb::demo_scan(0) } else { usb::scan() };
    let rows = usb::flatten(&devices, &HashSet::new());
    let rails = crate::ui::rails(&rows);
    for (r, &(_, i)) in rows.iter().enumerate() {
        let d = &devices[i];
        println!(
            "{}{} {} {:04x}:{:04x} [{}] {}",
            rails[r],
            d.name,
            d.icon(),
            d.vid,
            d.pid,
            d.class_name(),
            d.label()
        );
    }
}
