mod app;
mod cli;
mod events;
mod metrics;
mod pci;
mod ui;
mod usb;

use app::App;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};

pub static ICON_THEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
pub static COLOR_THEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(6);

fn main() -> std::io::Result<()> {
    // Reject unknown args up front — silently falling through to the TUI makes a
    // typo (or a stale binary that predates a flag) look like the flag did nothing.
    let args: Vec<_> = std::env::args().skip(1).collect();
    let theme = cli::parse_theme(args.iter().map(String::as_str)).unwrap_or_else(|error| {
        eprintln!("usbtree: {error}\ntry `usbtree --help`");
        std::process::exit(2);
    });
    if std::env::args().any(|a| a == "--version") {
        cli::version();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        cli::help();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--upgrade") {
        return cli::upgrade();
    }
    if std::env::args().any(|a| a == "--nerd-font" || a == "--nerd-fonts") {
        ICON_THEME.store(1, std::sync::atomic::Ordering::Relaxed);
    } else if std::env::args().any(|a| a == "--ascii") {
        ICON_THEME.store(2, std::sync::atomic::Ordering::Relaxed);
    }
    let demo = std::env::args().any(|a| a == "--demo");
    if std::env::args().any(|a| a == "--pci") {
        pci::dump();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--dump") {
        cli::dump(demo);
        return Ok(());
    }
    if std::env::args().any(|a| a == "--updatelist" || a == "--update-list") {
        match usb::update_list() {
            Ok((vendors, products, path)) => {
                println!("usb.ids updated: {vendors} vendors, {products} products");
                println!("saved to {}", path.display());
            }
            Err(e) => {
                eprintln!("updatelist failed: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // Read after the no-TUI exits above, so a bad USBTREE_THEME never breaks --dump.
    let env_theme = std::env::var_os("USBTREE_THEME");
    let env_theme = env_theme.as_ref().map(|value| value.to_string_lossy());
    let theme = cli::resolve_theme(theme, env_theme.as_deref()).unwrap_or_else(|error| {
        eprintln!("usbtree: {error}\ntry `usbtree --help`");
        std::process::exit(2);
    });
    if let Some(theme) = theme {
        COLOR_THEME.store(theme, std::sync::atomic::Ordering::Relaxed);
    }
    let mut terminal = ratatui::init();
    let _ = ratatui::crossterm::execute!(std::io::stdout(), EnableMouseCapture);
    let mut app = App::new(demo);
    let result = events::run(&mut app, &mut terminal);
    let _ = ratatui::crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests;
