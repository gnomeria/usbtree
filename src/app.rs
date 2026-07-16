use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;

use crate::metrics::Metrics;
use crate::pci::{self, PciDevice};
use crate::usb::{self, Device};

use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::layout::Position;
use crate::ui::clip;

use crate::ui::{theme, row_at, HISTORY};
use ratatui::style::Style;


pub const RESCAN_INTERVAL: Duration = Duration::from_secs(1);
pub const HIGHLIGHT_TTL: Duration = Duration::from_secs(30);

pub struct LogEvent {
    pub line: Line<'static>,
    pub id: String,
    pub name: String,
}

fn event_entry(stamp: &str, added: bool, d: &Device) -> LogEvent {
    let (glyph, color) = if added {
        ("▲ ", theme::MINT)
    } else {
        ("▼ ", theme::ROSE)
    };
    let line = Line::from(vec![
        stamp.to_string().fg(theme::DIM),
        Span::styled(glyph, Style::new().fg(color).bold()),
        format!("{:<8}", d.name).fg(theme::DIM),
        format!(" {} {}", d.icon(), d.label()).fg(theme::TEXT),
        format!("  {:04x}:{:04x}", d.vid, d.pid).fg(theme::FAINT),
    ]);
    LogEvent {
        line,
        id: format!("{:04x}:{:04x}", d.vid, d.pid),
        name: d.label(),
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum Focus {
    Tree,
    Events,
}

/// Which device bus the TUI is showing. USB is the full live view; PCI is a
/// read-only bus-grouped tree (`p` toggles).
#[derive(PartialEq, Clone, Copy)]
pub enum Tab {
    Usb,
    Pci,
}

/// One row of the PCI tree: a "segment:bus" header, or a device (index into
/// `App::pci`).
pub enum PciRow {
    Bus(String),
    Dev(usize),
}

/// A stable handle to the selected PCI row, snapshotted before a rescan so the
/// selection can be restored by identity (device address / bus name) rather than
/// row index, which shifts as devices come and go.
pub enum PciSel {
    Dev(String),
    Bus(String),
}

/// Live tree filter (opened with `/`). `editing` = keystrokes go to `query`;
/// once committed (Enter) the filter stays applied while you navigate.
pub struct Filter {
    pub query: String,
    pub editing: bool,
}

/// True if `d` matches the lowercased `q` on any human-facing field.
pub fn device_matches(d: &Device, q: &str) -> bool {
    d.name.to_lowercase().contains(q)
        || d.label().to_lowercase().contains(q)
        || d.vendor_name().to_lowercase().contains(q)
        || d.class_name().to_lowercase().contains(q)
        || format!("{:04x}:{:04x}", d.vid, d.pid).contains(q)
}

/// Keep every matched row plus its ancestor chain and full subtree, so matches
/// stay anchored in the tree instead of floating parentless.
pub fn visible_rows(rows: &[(usize, usize)], matched: &[bool]) -> Vec<(usize, usize)> {
    let mut keep = vec![false; rows.len()];
    for r in 0..rows.len() {
        if !matched[r] {
            continue;
        }
        keep[r] = true;
        let depth = rows[r].0;
        // ancestors: walk back, taking each row whose depth drops below the last
        let mut d = depth;
        for pr in (0..r).rev() {
            let pd = rows[pr].0;
            if pd < d {
                keep[pr] = true;
                d = pd;
                if pd == 0 {
                    break;
                }
            }
        }
        // subtree: following rows deeper than this one
        for sr in (r + 1)..rows.len() {
            if rows[sr].0 > depth {
                keep[sr] = true;
            } else {
                break;
            }
        }
    }
    rows.iter()
        .zip(keep)
        .filter_map(|(&row, k)| k.then_some(row))
        .collect()
}

/// Tree rows to show: honor the collapse set when idle, but a non-empty filter
/// query flattens the whole tree so matches inside collapsed hubs surface,
/// with `visible_rows` re-anchoring each match to its ancestor chain.
pub fn filtered_rows(render: &[Device], collapsed: &HashSet<String>, query: &str) -> Vec<(usize, usize)> {
    let q = query.to_lowercase();
    if q.is_empty() {
        return usb::flatten(render, collapsed);
    }
    let rows = usb::flatten(render, &HashSet::new());
    let matched: Vec<bool> = rows
        .iter()
        .map(|&(_, i)| device_matches(&render[i], &q))
        .collect();
    visible_rows(&rows, &matched)
}

/// Right-click copy menu. Items are (label, clipboard text, toast noun).
pub struct ContextMenu {
    pub rect: Rect,
    pub items: Vec<(String, String, String)>,
    pub hover: usize,
}

impl ContextMenu {
    /// Which item (if any) sits under the given screen cell.
    pub fn item_at(&self, col: u16, row: u16) -> Option<usize> {
        if col <= self.rect.x || col >= self.rect.right() - 1 {
            return None; // outside content columns (borders)
        }
        let i = row.checked_sub(self.rect.y + 1)? as usize;
        (i < self.items.len()).then_some(i)
    }
}



pub struct App {
    /// scripted fake devices + traffic (`--demo`)
    pub demo: bool,
    pub devices: Vec<Device>,
    /// devices + lingering ghosts of removed ones; what the tree shows
    pub render: Vec<Device>,
    pub rows: Vec<(usize, usize)>, // (depth, index into render)
    pub flash: HashMap<String, Instant>,
    pub ghosts: Vec<(Device, Instant)>,
    pub collapsed: HashSet<String>,
    pub list: ListState,
    pub log: VecDeque<LogEvent>,
    pub log_state: ListState,
    pub focus: Focus,
    pub started: Instant,
    pub last_scan: Instant,
    pub metrics: Metrics,
    /// per-device activity history, newest last
    pub rates: HashMap<String, Vec<u64>>,
    /// transient status line (e.g. "copied …"): (message, raised_at, ok) — `ok`
    /// drives the ✓/✗ glyph, set by the producer so the UI never sniffs the text
    pub toast: Option<(String, Instant, bool)>,
    /// label of the drive currently being ejected off-thread, if any; shown as a
    /// persistent "ejecting…" line so a slow power-off never freezes the UI
    pub ejecting: Option<String>,
    /// channel carrying the eject result back from its worker thread
    pub eject_rx: Option<Receiver<Result<String, String>>>,
    /// newer release version once the background check finds one (no auto-update)
    pub update: Option<String>,
    /// one-shot channel carrying the newer version from the check thread
    pub update_rx: Option<Receiver<String>>,
    /// full frame + pane rects from the last draw, for mapping mouse cells to rows
    pub screen: Rect,
    pub tree_rect: Rect,
    pub log_rect: Rect,
    /// open right-click copy menu, if any
    pub menu: Option<ContextMenu>,
    /// live tree filter (`/`), if any
    pub filter: Option<Filter>,
    /// open eject confirmation dialog: the sysfs name pending eject
    pub confirm: Option<String>,
    /// names ejected in `--demo`: real eject is unavailable, so instead we drop
    /// these from every synthetic scan to visibly play the unplug (ghost + log)
    // ponytail: stays gone for the session; the loop won't replug it. Fine for a
    // demo — clear on the device's next natural unplug if that ever matters.
    pub demo_ejected: HashSet<String>,
    /// which bus view is shown (USB / PCI)
    pub tab: Tab,
    /// PCI devices (flat, address-sorted); only refreshed while the PCI tab is up
    pub pci: Vec<PciDevice>,
    /// computed PCI tree rows (bus headers + devices), collapse/filter applied
    pub pci_rows: Vec<PciRow>,
    pub pci_list: ListState,
}

impl App {
    pub fn new(demo: bool) -> Self {
        let devices = if demo { usb::demo_scan(0) } else { usb::scan() };
        let rows = usb::flatten(&devices, &HashSet::new());
        let mut list = ListState::default();
        if !rows.is_empty() {
            list.select(Some(0));
        }
        let mut metrics = if demo {
            Metrics::demo()
        } else {
            Metrics::new()
        };
        metrics.sample(&devices); // baseline so the first tick is a delta, not a total
        // check GitHub for a newer release off the UI thread; skip in demo so
        // screenshots/VHS stay offline and deterministic
        let update_rx = (!demo).then(|| {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                if let Some(v) = usb::latest_release()
                    && usb::is_newer(&v, env!("CARGO_PKG_VERSION"))
                {
                    let _ = tx.send(v);
                }
            });
            rx
        });
        Self {
            update: None,
            update_rx,
            demo,
            render: devices.clone(),
            devices,
            rows,
            flash: HashMap::new(),
            ghosts: Vec::new(),
            collapsed: HashSet::new(),
            list,
            log: VecDeque::new(),
            log_state: ListState::default(),
            focus: Focus::Tree,
            started: Instant::now(),
            last_scan: Instant::now(),
            metrics,
            rates: HashMap::new(),
            toast: None,
            ejecting: None,
            eject_rx: None,
            screen: Rect::default(),
            tree_rect: Rect::default(),
            log_rect: Rect::default(),
            menu: None,
            filter: None,
            confirm: None,
            demo_ejected: HashSet::new(),
            tab: Tab::Usb,
            pci: Vec::new(),
            pci_rows: Vec::new(),
            pci_list: ListState::default(),
        }
    }

    pub fn set_focus(&mut self, f: Focus) {
        self.focus = f;
        match f {
            Focus::Events if self.log_state.selected().is_none() && !self.log.is_empty() => {
                self.log_state.select(Some(0))
            }
            Focus::Tree => self.log_state.select(None),
            _ => {}
        }
    }

    pub fn toggle_focus(&mut self) {
        self.set_focus(match self.focus {
            Focus::Tree => Focus::Events,
            Focus::Events => Focus::Tree,
        });
    }

    pub fn open_filter(&mut self) {
        self.menu = None;
        self.set_focus(Focus::Tree);
        let query = self.filter.take().map(|f| f.query).unwrap_or_default();
        self.filter = Some(Filter { query, editing: true });
    }

    pub fn clear_filter(&mut self) {
        self.filter = None;
        self.rebuild_rows();
    }

    pub fn toggle_tab(&mut self) {
        self.menu = None;
        self.tab = match self.tab {
            Tab::Usb => Tab::Pci,
            Tab::Pci => Tab::Usb,
        };
        if self.tab == Tab::Pci {
            self.pci_rescan();
            if self.pci_list.selected().is_none() && !self.pci_rows.is_empty() {
                self.pci_list.select(Some(0));
            }
        }
    }

    pub fn pci_rescan(&mut self) {
        let keep = self.pci_selected_key();
        self.pci = if self.demo { pci::demo_scan() } else { pci::scan() };
        self.rebuild_pci_rows_keeping(keep);
    }

    pub fn pci_selected(&self) -> Option<usize> {
        match self.pci_rows.get(self.pci_list.selected()?)? {
            PciRow::Dev(i) => Some(*i),
            PciRow::Bus(_) => None,
        }
    }

    pub fn pci_selected_key(&self) -> Option<PciSel> {
        match self.pci_rows.get(self.pci_list.selected()?)? {
            PciRow::Dev(i) => Some(PciSel::Dev(self.pci[*i].addr.clone())),
            PciRow::Bus(b) => Some(PciSel::Bus(b.clone())),
        }
    }

    pub fn compute_pci_rows(&self) -> Vec<PciRow> {
        let q = self.filter.as_ref().map(|f| f.query.to_lowercase());
        let mut rows = Vec::new();
        let mut cur = String::new();
        for (i, d) in self.pci.iter().enumerate() {
            if let Some(q) = &q
                && !q.is_empty()
                && !d.matches(q)
            {
                continue;
            }
            if d.bus() != cur {
                cur = d.bus().to_string();
                rows.push(PciRow::Bus(cur.clone()));
            }
            rows.push(PciRow::Dev(i));
        }
        rows
    }

    pub fn rebuild_pci_rows(&mut self) {
        let keep = self.pci_selected_key();
        self.rebuild_pci_rows_keeping(keep);
    }

    pub fn rebuild_pci_rows_keeping(&mut self, keep: Option<PciSel>) {
        let prev = self.pci_list.selected();
        self.pci_rows = self.compute_pci_rows();
        if self.pci_rows.is_empty() {
            self.pci_list.select(None);
            return;
        }
        let pos = keep.and_then(|k| {
            self.pci_rows.iter().position(|r| match (&k, r) {
                (PciSel::Dev(a), PciRow::Dev(i)) => &self.pci[*i].addr == a,
                (PciSel::Bus(b), PciRow::Bus(s)) => s == b,
                _ => false,
            })
        });
        let fallback = prev.unwrap_or(0).min(self.pci_rows.len() - 1);
        self.pci_list.select(Some(pos.unwrap_or(fallback)));
    }

    pub fn filter_key(&mut self, code: KeyCode) {
        match code {
            // arrows still navigate the live results without touching the query
            KeyCode::Up => return self.nav(-1),
            KeyCode::Down => return self.nav(1),
            KeyCode::Char(c) => {
                if let Some(f) = &mut self.filter {
                    f.query.push(c)
                }
            }
            KeyCode::Backspace => {
                if let Some(f) = &mut self.filter {
                    f.query.pop();
                }
            }
            KeyCode::Enter => match &mut self.filter {
                Some(f) if f.query.is_empty() => self.filter = None,
                Some(f) => f.editing = false, // commit: keep filtering, leave input
                None => {}
            },
            KeyCode::Esc => self.filter = None,
            _ => return,
        }
        self.rebuild_rows();
    }

    pub fn rebuild_rows(&mut self) {
        if self.tab == Tab::Pci {
            return self.rebuild_pci_rows();
        }
        let name = self
            .list
            .selected()
            .and_then(|s| self.rows.get(s))
            .map(|&(_, i)| self.render[i].name.clone());
        self.rows = self.compute_rows();
        let pos = name.and_then(|n| self.rows.iter().position(|&(_, i)| self.render[i].name == n));
        self.list
            .select((!self.rows.is_empty()).then(|| pos.unwrap_or(0)));
    }

    pub fn compute_rows(&self) -> Vec<(usize, usize)> {
        let query = self.filter.as_ref().map_or("", |f| f.query.as_str());
        filtered_rows(&self.render, &self.collapsed, query)
    }

    pub fn on_mouse(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::Down(MouseButton::Right) => self.open_menu(m.column, m.row),
            MouseEventKind::Down(MouseButton::Left) => self.click(m.column, m.row),
            MouseEventKind::ScrollDown => self.scroll_at(m.column, m.row, 1),
            MouseEventKind::ScrollUp => self.scroll_at(m.column, m.row, -1),
            MouseEventKind::Moved => {
                if let Some(menu) = &mut self.menu
                    && let Some(i) = menu.item_at(m.column, m.row)
                {
                    menu.hover = i;
                }
            }
            _ => {}
        }
    }

    pub fn tree_row_at(&self, col: u16, row: u16) -> Option<usize> {
        row_at(self.tree_rect, self.list.offset(), self.rows.len(), col, row)
    }

    pub fn log_row_at(&self, col: u16, row: u16) -> Option<usize> {
        row_at(self.log_rect, self.log_state.offset(), self.log.len(), col, row)
    }

    pub fn click(&mut self, col: u16, row: u16) {
        if let Some(menu) = self.menu.take() {
            if let Some(i) = menu.item_at(col, row) {
                let (_, text, what) = &menu.items[i];
                self.copy(&text.clone(), &what.clone());
            }
            return; // click outside the menu just dismisses it
        }
        if let Some(idx) = self.tree_row_at(col, row) {
            self.set_focus(Focus::Tree);
            self.list.select(Some(idx));
        } else if let Some(idx) = self.log_row_at(col, row) {
            self.set_focus(Focus::Events);
            self.log_state.select(Some(idx));
        }
    }

    pub fn scroll_at(&mut self, col: u16, row: u16, delta: isize) {
        self.menu = None;
        let pos = Position::new(col, row);
        let target = if self.tree_rect.contains(pos) {
            Focus::Tree
        } else if self.log_rect.contains(pos) {
            Focus::Events
        } else {
            return;
        };
        self.set_focus(target);
        self.nav(delta);
    }

    pub fn open_menu(&mut self, col: u16, row: u16) {
        let items = if let Some(idx) = self.tree_row_at(col, row) {
            self.set_focus(Focus::Tree);
            self.list.select(Some(idx));
            let (_, i) = self.rows[idx];
            let d = &self.render[i];
            let id = format!("{:04x}:{:04x}", d.vid, d.pid);
            let mut items: Vec<(String, String, String)> = vec![
                ("vid:pid".into(), id.clone(), id.clone()),
                ("name".into(), d.label(), "name".into()),
                ("sysfs path".into(), d.name.clone(), "sysfs path".into()),
            ];
            if let Some(s) = &d.serial {
                items.push(("serial".into(), s.clone(), "serial".into()));
            }
            let mut block = format!("{}\n{id}\n{}", d.label(), d.name);
            if let Some(s) = &d.serial {
                block.push_str(&format!("\n{s}"));
            }
            items.push(("full details".into(), block, format!("{} details", d.name)));
            items
        } else if let Some(idx) = self.log_row_at(col, row) {
            self.set_focus(Focus::Events);
            self.log_state.select(Some(idx));
            let ev = &self.log[idx];
            vec![
                ("id".into(), ev.id.clone(), "event id".into()),
                ("name".into(), ev.name.clone(), "event name".into()),
            ]
        } else {
            self.menu = None;
            return;
        };
        // size to the widest label, then clamp so the box stays on screen
        let w = items.iter().map(|(l, _, _)| l.chars().count()).max().unwrap_or(4) as u16 + 4;
        let h = items.len() as u16 + 2;
        let x = col.min(self.screen.right().saturating_sub(w));
        let y = row.min(self.screen.bottom().saturating_sub(h));
        self.menu = Some(ContextMenu {
            rect: Rect { x, y, width: w, height: h },
            items,
            hover: 0,
        });
    }

    pub fn copy(&mut self, text: &str, what: &str) {
        let (msg, ok) = match clip(text) {
            Ok(()) => (format!("copied {what}"), true),
            Err(e) => (format!("copy failed: {e}"), false),
        };
        self.toast = Some((msg, Instant::now(), ok));
    }

    pub fn nav(&mut self, delta: isize) {
        let (state, len) = match (self.tab, self.focus) {
            (Tab::Pci, _) => (&mut self.pci_list, self.pci_rows.len()),
            (_, Focus::Tree) => (&mut self.list, self.rows.len()),
            (_, Focus::Events) => (&mut self.log_state, self.log.len()),
        };
        if len == 0 {
            return;
        }
        let cur = state.selected().unwrap_or(0) as isize;
        let new = (cur + delta).clamp(0, len as isize - 1);
        state.select(Some(new as usize));
    }

    pub fn nav_to(&mut self, idx: isize) {
        let (state, len) = match (self.tab, self.focus) {
            (Tab::Pci, _) => (&mut self.pci_list, self.pci_rows.len()),
            (_, Focus::Tree) => (&mut self.list, self.rows.len()),
            (_, Focus::Events) => (&mut self.log_state, self.log.len()),
        };
        if len == 0 {
            return;
        }
        let clamped = idx.clamp(0, len as isize - 1);
        state.select(Some(clamped as usize));
    }

    pub fn yank(&mut self, full: bool) {
        if self.focus == Focus::Events {
            let Some(ev) = self.log_state.selected().and_then(|s| self.log.get(s)) else {
                return;
            };
            let (text, what) = if full {
                (ev.name.clone(), "event name")
            } else {
                (ev.id.clone(), "event id")
            };
            self.copy(&text, what);
            return;
        }
        let Some(&(_, i)) = self.list.selected().and_then(|s| self.rows.get(s)) else {
            return;
        };
        let d = &self.render[i];
        let id = format!("{:04x}:{:04x}", d.vid, d.pid);
        let (text, what) = if full {
            let mut t = format!("{}\n{id}\n{}", d.label(), d.name);
            if let Some(s) = &d.serial {
                t.push_str(&format!("\n{s}"));
            }
            (t, format!("{} details", d.name))
        } else {
            (id.clone(), id)
        };
        self.copy(&text, &what);
    }

    pub fn can_eject(&self) -> bool {
        self.tab == Tab::Usb
            && self
                .list
                .selected()
                .and_then(|s| self.rows.get(s))
                .is_some_and(|&(_, i)| self.render[i].effective_class() == 0x08)
    }

    pub fn eject_key(&mut self) {
        // one eject at a time — ignore `e` while a power-off is still in flight
        if !self.can_eject() || self.ejecting.is_some() {
            return;
        }
        let Some(&(_, i)) = self.list.selected().and_then(|s| self.rows.get(s)) else {
            return;
        };
        self.menu = None;
        self.confirm = Some(self.render[i].name.clone());
    }

    pub fn confirm_key(&mut self, code: KeyCode) {
        let name = self.confirm.take();
        if let (KeyCode::Char('e' | 'y') | KeyCode::Enter, Some(name)) = (code, name) {
            if self.demo {
                // no real hardware to eject — drop it from the synthetic scan so
                // the tree plays the unplug (ghost + event-log entry) for real
                self.demo_ejected.insert(name.clone());
                self.toast = Some((format!("ejected {name}"), Instant::now(), true));
                self.rescan();
                return;
            }
            let label = self
                .render
                .iter()
                .find(|d| d.name == name)
                .map(|d| d.label())
                .unwrap_or_else(|| name.clone());
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(usb::eject(&name));
            });
            self.eject_rx = Some(rx);
            self.ejecting = Some(label);
        }
    }

    pub fn rescan(&mut self) {
        self.last_scan = Instant::now();
        if self.tab == Tab::Pci {
            self.pci_rescan();
        }
        let mut new = if self.demo {
            usb::demo_scan(self.started.elapsed().as_secs())
        } else {
            usb::scan()
        };
        if self.demo {
            new.retain(|d| !self.demo_ejected.contains(&d.name));
        }
        let (added, removed) = usb::diff(&self.devices, &new);
        let stamp = self.started.elapsed().as_secs();
        let stamp = format!("[{:02}:{:02}] ", stamp / 60, stamp % 60);
        for d in &added {
            self.log.push_front(event_entry(&stamp, true, d));
        }
        for d in &removed {
            self.log.push_front(event_entry(&stamp, false, d));
        }
        self.log.truncate(200);
        let now = Instant::now();
        for d in &added {
            self.flash.insert(d.name.clone(), now);
            self.ghosts.retain(|(g, _)| g.name != d.name);
        }
        let removed: Vec<Device> = removed.into_iter().cloned().collect();
        for d in removed {
            self.ghosts.retain(|(g, _)| g.name != d.name);
            self.ghosts.push((d, now));
        }
        self.devices = new;
        self.flash.retain(|_, t| t.elapsed() < HIGHLIGHT_TTL);
        self.ghosts.retain(|(_, t)| t.elapsed() < HIGHLIGHT_TTL);

        let rates = self.metrics.sample(&self.devices);
        for d in &self.devices {
            let h = self.rates.entry(d.name.clone()).or_default();
            h.push(rates.get(&d.name).copied().unwrap_or(0));
            if h.len() > HISTORY {
                h.remove(0);
            }
        }
        // keep history for present devices and still-fading ghosts (frozen old data)
        self.rates.retain(|k, _| {
            self.devices.iter().any(|d| &d.name == k)
                || self.ghosts.iter().any(|(d, _)| &d.name == k)
        });

        // keep selection on the same device across rescans
        let selected_name = self
            .list
            .selected()
            .and_then(|s| self.rows.get(s))
            .map(|&(_, i)| self.render[i].name.clone());
        self.render = self.devices.clone();
        self.render
            .extend(self.ghosts.iter().map(|(d, _)| d.clone()));
        self.rows = self.compute_rows();
        let sel = selected_name
            .and_then(|n| {
                self.rows
                    .iter()
                    .position(|&(_, i)| self.render[i].name == n)
            })
            .unwrap_or(0);
        if !self.rows.is_empty() {
            self.list.select(Some(sel));
        } else {
            self.list.select(None);
        }
    }

    pub fn ghost_age(&self, name: &str) -> Option<f32> {
        self.ghosts
            .iter()
            .find(|(d, _)| d.name == name)
            .map(|(_, t)| t.elapsed().as_secs_f32() / HIGHLIGHT_TTL.as_secs_f32())
    }

    pub fn flash_age(&self, name: &str) -> Option<f32> {
        self.flash
            .get(name)
            .map(|t| t.elapsed().as_secs_f32() / HIGHLIGHT_TTL.as_secs_f32())
    }

    pub fn fold(&mut self, want: Option<bool>) {
        let Some(&(_, i)) = self.list.selected().and_then(|s| self.rows.get(s)) else {
            return;
        };
        let name = self.render[i].name.clone();
        if usb::child_count(&self.render, &name) == 0 {
            return;
        }
        let folded = self.collapsed.contains(&name);
        if want.unwrap_or(!folded) == folded {
            return;
        }
        if folded {
            self.collapsed.remove(&name);
        } else {
            self.collapsed.insert(name.clone());
        }
        self.rows = self.compute_rows();
        if let Some(pos) = self
            .rows
            .iter()
            .position(|&(_, j)| self.render[j].name == name)
        {
            self.list.select(Some(pos));
        }
    }
}
