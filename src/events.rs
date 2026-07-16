use std::time::Instant;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crate::app::{App, Tab, RESCAN_INTERVAL};

    pub fn run(app: &mut App, terminal: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(|f| crate::ui::draw(app, f))?;
            // ponytail: 1s enumeration poll — switch to nusb::watch_devices()
            // hotplug events if latency matters
            if event::poll(RESCAN_INTERVAL.saturating_sub(app.last_scan.elapsed()))? {
                match event::read()? {
                    // typing into the `/` filter grabs keys before any binding
                    Event::Key(key)
                        if key.kind == KeyEventKind::Press
                            && app.filter.as_ref().is_some_and(|f| f.editing) =>
                    {
                        app.filter_key(key.code)
                    }
                    // eject dialog is modal: it swallows keys until confirmed/cancelled
                    Event::Key(key)
                        if key.kind == KeyEventKind::Press && app.confirm.is_some() =>
                    {
                        app.confirm_key(key.code)
                    }
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        // any keypress dismisses an open menu; Esc then does nothing else
                        let menu_was_open = app.menu.take().is_some();
                        match key.code {
                            KeyCode::Esc if menu_was_open => {}
                            KeyCode::Esc if app.filter.is_some() => app.clear_filter(),
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char('/') => app.open_filter(),
                            KeyCode::Char('p') => app.toggle_tab(),
                            KeyCode::Tab | KeyCode::BackTab if app.tab == Tab::Usb => {
                                app.toggle_focus()
                            }
                            KeyCode::Down | KeyCode::Char('j') => app.nav(1),
                            KeyCode::Up | KeyCode::Char('k') => app.nav(-1),
                            KeyCode::Char('g') | KeyCode::Home => app.nav_to(0),
                            KeyCode::Char('G') | KeyCode::End => app.nav_to(isize::MAX),
                            KeyCode::Enter | KeyCode::Char(' ') if app.tab == Tab::Usb => {
                                app.fold(None)
                            }
                            KeyCode::Left | KeyCode::Char('h') if app.tab == Tab::Usb => {
                                app.fold(Some(true))
                            }
                            KeyCode::Right | KeyCode::Char('l') if app.tab == Tab::Usb => {
                                app.fold(Some(false))
                            }
                            KeyCode::Char('r') => app.rescan(),
                            KeyCode::Char('y') if app.tab == Tab::Usb => app.yank(false),
                            KeyCode::Char('Y') if app.tab == Tab::Usb => app.yank(true),
                            KeyCode::Char('e') if app.tab == Tab::Usb => app.eject_key(),
                            _ => {}
                        }
                    }
                    Event::Mouse(m) if app.tab == Tab::Usb => app.on_mouse(m),
                    _ => {}
                }
            }
            if app.last_scan.elapsed() >= RESCAN_INTERVAL {
                app.rescan();
            }
            if let Some(rx) = &app.update_rx
                && let Ok(v) = rx.try_recv()
            {
                app.update = Some(v);
                app.update_rx = None;
            }
            if let Some(rx) = &app.eject_rx
                && let Ok(res) = rx.try_recv()
            {
                app.eject_rx = None;
                app.ejecting = None;
                let ok = res.is_ok();
                app.toast = Some((res.unwrap_or_else(|e| e), Instant::now(), ok));
                app.rescan();
            }
        }
    }
