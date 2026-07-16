mod app;
mod cli;
mod events;
mod metrics;
mod pci;
mod ui;
mod usb;

use app::App;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};

fn main() -> std::io::Result<()> {
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
