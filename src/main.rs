mod app;
mod cli;
mod events;
mod metrics;
mod pci;
mod report;
mod ui;
mod usb;

use app::App;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};

pub static ICON_THEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
pub static COLOR_THEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(6);

fn main() -> std::io::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|a| a == "--nerd-font" || a == "--nerd-fonts") {
        ICON_THEME.store(1, std::sync::atomic::Ordering::Relaxed);
    } else if args.iter().any(|a| a == "--ascii") {
        ICON_THEME.store(2, std::sync::atomic::Ordering::Relaxed);
    }
    if args.iter().any(|a| a == "--light") {
        COLOR_THEME.store(3, std::sync::atomic::Ordering::Relaxed);
    }
    let demo = args.iter().any(|a| a == "--demo");
    if args.iter().any(|a| a == "--pci") {
        pci::dump();
        return Ok(());
    }
    if args.iter().any(|a| a == "--dump") {
        cli::dump(demo);
        return Ok(());
    }
    if args.iter().any(|a| a == "--json") {
        cli::json(demo);
        return Ok(());
    }
    if args.iter().any(|a| a == "--markdown") {
        cli::markdown(demo);
        return Ok(());
    }
    if args.iter().any(|a| a == "--snapshot") {
        let Some(path) = cli::flag_value(&args, "--snapshot") else {
            eprintln!("--snapshot needs a path, e.g. usbtree --snapshot before.json");
            std::process::exit(2);
        };
        cli::snapshot(demo, path)?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--diff") {
        let Some(path) = cli::flag_value(&args, "--diff") else {
            eprintln!("--diff needs a snapshot path, e.g. usbtree --diff before.json");
            std::process::exit(2);
        };
        cli::diff(demo, path)?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--updatelist" || a == "--update-list") {
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
