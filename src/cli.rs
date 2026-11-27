use crate::usb;
use std::collections::HashSet;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Switches that take no value.
/// Keep in sync with `help()` and the dispatch in `main.rs`.
const KNOWN_FLAGS: &[&str] = &[
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

pub fn parse_theme<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<Option<u8>, String> {
    let mut theme = None;
    while let Some(arg) = args.next() {
        let name = match arg {
            "--light" => "light",
            "--theme" => args.next().filter(|s| !s.starts_with('-'))
                .ok_or("--theme requires a theme name")?,
            _ if KNOWN_FLAGS.contains(&arg) => continue,
            _ => arg.strip_prefix("--theme=")
                .ok_or_else(|| format!("unknown option `{arg}`"))?,
        };
        let lowercase = name.to_ascii_lowercase();
        let lookup = match lowercase.as_str() {
            "auto" => "terminal",
            "light" => "latte",
            "dark" => "tokyo-night",
            name => name,
        };
        let index = crate::ui::theme::THEME_NAMES.iter()
            .position(|&(key, label)| key == lookup || label.eq_ignore_ascii_case(lookup))
            .ok_or_else(|| format!("unknown theme `{name}`"))?;
        theme = Some(index as u8);
    }
    Ok(theme)
}

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
  --theme NAME  select the startup theme
  --light       select Latte (also --theme light or --theme latte)
  --nerd-font   nerd-font icons
  --ascii       ascii-only icons

Theme names:
  {}
  Aliases: auto = terminal, light = latte, dark = tokyo-night
  Picker labels also work, e.g. --theme \"Tokyo Night\". Names ignore case.",
        crate::ui::theme::THEME_NAMES.map(|(key, _)| key).join(" ")
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
    // Pick the demo frame with the SSD attached for the one-shot dump.
    let devices = if demo { usb::demo_scan(10) } else { usb::scan() };
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
        if d.storage.is_empty() {
            continue;
        }
        let mut indent = rails[r].replace("├─ ", "│  ").replace("└─ ", "   ");
        if rows.get(r + 1).is_some_and(|next| next.0 > rows[r].0) {
            indent.push('│');
        }
        for node in &d.storage {
            println!("{indent}  {}", node.path);
            for mount in &node.mounts {
                println!("{indent}    mounted at {mount}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_theme;

    #[test]
    fn every_theme_accepts_key_and_picker_label() {
        for (index, (key, label)) in crate::ui::theme::THEME_NAMES.iter().enumerate() {
            for name in [*key, *label, &key.to_ascii_uppercase()] {
                assert_eq!(parse_theme(["--theme", name].into_iter()), Ok(Some(index as u8)), "{name}");
            }
        }
        assert_eq!(parse_theme(["--theme=DARK"].into_iter()), Ok(Some(6)));
    }

    #[test]
    fn auto_theme_selects_terminal() {
        assert_eq!(parse_theme(["--theme", "auto"].into_iter()), Ok(Some(0)));
        assert_eq!(parse_theme(["--theme=auto"].into_iter()), Ok(Some(0)));
    }

    #[test]
    fn light_theme_alias_selects_latte() {
        let latte = parse_theme(["--theme", "latte"].into_iter());
        assert_eq!(latte, Ok(Some(3)));
        assert_eq!(parse_theme(["--theme", "light"].into_iter()), latte);
        assert_eq!(parse_theme(["--theme=light"].into_iter()), latte);
    }

    #[test]
    fn last_theme_option_selects_palette() {
        let args = ["--light", "--theme", "tokyo-night"];
        assert_eq!(parse_theme(args.into_iter()), Ok(Some(6)));

        let args = ["--theme=terminal", "--light"];
        assert_eq!(parse_theme(args.into_iter()), Ok(Some(3)));

        assert_eq!(parse_theme(["--theme", "terminal"].into_iter()), Ok(Some(0)));
    }
}
