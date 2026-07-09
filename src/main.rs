mod metrics;
mod pci;
mod usb;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEvent, MouseEventKind,
};
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Clear, List, ListItem, ListState, Padding, Paragraph, Sparkline,
};

use metrics::Metrics;
use pci::PciDevice;
use usb::Device;

const RESCAN_INTERVAL: Duration = Duration::from_secs(1);
/// How long added devices stay highlighted / removed devices linger in the tree.
const HIGHLIGHT_TTL: Duration = Duration::from_secs(30);

/// Charm/lipgloss-inspired pastel palette (truecolor).
mod theme {
    use ratatui::style::Color;

    pub const ACCENT: Color = Color::Rgb(0xb4, 0x8e, 0xff); // lavender
    pub const PILL: Color = Color::Rgb(0x7d, 0x56, 0xf4); // charm purple
    pub const PILL_FG: Color = Color::Rgb(0xf8, 0xf8, 0xfc);
    pub const TEXT: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
    pub const DIM: Color = Color::Rgb(0x6c, 0x70, 0x86);
    pub const FAINT: Color = Color::Rgb(0x45, 0x47, 0x5a);
    pub const BORDER: Color = Color::Rgb(0x36, 0x38, 0x4e);
    pub const SURFACE: Color = Color::Rgb(0x2e, 0x30, 0x45);
    pub const SEL_BG: Color = Color::Rgb(0x2a, 0x2c, 0x40);
    pub const MINT: Color = Color::Rgb(0xa6, 0xe3, 0xa1);
    pub const ROSE: Color = Color::Rgb(0xf3, 0x8b, 0xa8);
    pub const BLUE: Color = Color::Rgb(0x89, 0xb4, 0xfa);
    pub const TEAL: Color = Color::Rgb(0x94, 0xe2, 0xd5);
    pub const YELLOW: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
    pub const PEACH: Color = Color::Rgb(0xfa, 0xb3, 0x87);
    pub const MAUVE: Color = Color::Rgb(0xcb, 0xa6, 0xf7);
    pub const GREEN: Color = Color::Rgb(0xa6, 0xda, 0x95);
    pub const SKY: Color = Color::Rgb(0x74, 0xc7, 0xec);
}

/// One hue per device class, so color carries meaning across the whole UI.
fn class_color(class: u8) -> Color {
    match class {
        0x01 => theme::TEAL,                // audio
        0x02 | 0x0a | 0xe0 => theme::GREEN, // comm / wireless
        0x03 => theme::BLUE,                // HID
        0x06 | 0x0e | 0x10 => theme::MAUVE, // imaging / video
        0x07 => theme::PEACH,               // printer
        0x08 => theme::YELLOW,              // storage
        0x09 => theme::SKY,                 // hub
        _ => theme::TEXT,
    }
}

/// One hue per PCI base class (numbering differs from USB: 0x01 = storage,
/// 0x02 = network, 0x03 = display …).
fn pci_class_color(class: u8) -> Color {
    match class {
        0x01 => theme::YELLOW,        // mass storage
        0x02 => theme::GREEN,         // network
        0x03 => theme::MAUVE,         // display
        0x04 => theme::TEAL,          // multimedia
        0x06 => theme::SKY,           // bridge
        0x07 => theme::PEACH,         // comm
        0x0c => theme::BLUE,          // serial bus (USB/thunderbolt)
        0x0d => theme::GREEN,         // wireless
        _ => theme::TEXT,
    }
}

fn pci_icon(class: u8) -> &'static str {
    match class {
        0x01 => "💾",
        0x02 => "🌐",
        0x03 => "🖥️",
        0x04 => "🎬",
        0x06 => "🌉",
        0x07 => "📞",
        0x0c => "🔌",
        0x0d => "📶",
        _ => "🔹",
    }
}

/// (tier glyph, human-readable speed, color) — brighter with tier.
fn speed_badge(speed: &str) -> Option<(&'static str, String, Color)> {
    let mbps: f32 = speed.parse().ok()?;
    let (glyph, color) = if mbps >= 5000.0 {
        ("█", theme::ACCENT)
    } else if mbps >= 480.0 {
        ("▄", theme::SKY)
    } else {
        ("▂", theme::DIM)
    };
    let human = if mbps >= 1000.0 {
        format!("{}G", mbps / 1000.0)
    } else {
        format!("{}M", mbps)
    };
    Some((glyph, human, color))
}

/// How many activity samples to keep per device (1 per rescan tick).
const HISTORY: usize = 60;
/// Sparkline width in tree rows.
const SPARK_WIDTH: usize = 10;

const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Mini text sparkline of the last `width` samples, scaled to their max.
fn sparkline(samples: &[u64], width: usize) -> String {
    let tail = &samples[samples.len().saturating_sub(width)..];
    let max = tail.iter().copied().max().unwrap_or(0).max(1);
    tail.iter()
        .map(|&v| {
            if v == 0 {
                ' '
            } else {
                BARS[((v as f64 / max as f64) * 7.0).round() as usize]
            }
        })
        .collect()
}

fn fmt_rate(v: u64, bytes: bool) -> String {
    if !bytes {
        return format!("{v}/s");
    }
    match v {
        0..=1023 => format!("{v} B/s"),
        1024..=1048575 => format!("{:.1} K/s", v as f64 / 1024.0),
        1048576..=1073741823 => format!("{:.1} M/s", v as f64 / 1048576.0),
        _ => format!("{:.1} G/s", v as f64 / 1073741824.0),
    }
}

/// USB endpoint transfer-type name from `bmAttributes` bits 0-1.
fn ep_type(t: u8) -> &'static str {
    ["ctrl", "iso", "bulk", "int"][(t & 0x03) as usize]
}

/// BCD version word (bcdUSB / bcdDevice) as a dotted string: 0x0200 -> "2.00".
fn bcd(v: u16) -> String {
    format!("{:x}.{:02x}", v >> 8, v & 0xff)
}

/// Standard base64, no padding elided. ~15 lines beats an extra crate.
fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for c in input.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if c.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// Copy to the terminal clipboard via OSC 52 — works locally and over SSH,
/// no platform clipboard crate. Terminal must allow it (most do).
fn clip(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = std::io::stdout();
    write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes()))?;
    out.flush()
}

fn lerp(a: Color, b: Color, t: f32) -> Color {
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (a, b) else {
        return a;
    };
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
}

/// Tree rail prefix ("│  ├─ └─ ") per row, from the depth sequence.
// ponytail: O(n²) lookahead for last-sibling; fine at USB tree sizes
fn rails(rows: &[(usize, usize)]) -> Vec<String> {
    let is_last = |i: usize| {
        let d = rows[i].0;
        for &(dj, _) in &rows[i + 1..] {
            if dj < d {
                return true;
            }
            if dj == d {
                return false;
            }
        }
        true
    };
    let mut stack: Vec<bool> = Vec::new(); // last-sibling flags of open ancestors
    let mut out = Vec::with_capacity(rows.len());
    for (i, &(depth, _)) in rows.iter().enumerate() {
        if depth == 0 {
            stack.clear();
            out.push(String::new());
            continue;
        }
        stack.truncate(depth - 1);
        let last = is_last(i);
        let mut s = String::new();
        for &anc_last in &stack {
            s.push_str(if anc_last { "   " } else { "│  " });
        }
        s.push_str(if last { "└─ " } else { "├─ " });
        stack.push(last);
        out.push(s);
    }
    out
}

fn pane(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme::BORDER))
        .title(Line::from(format!(" {title} ").fg(theme::ACCENT).bold()))
        .padding(Padding::horizontal(1))
}

/// Map a screen cell to a list row inside a bordered pane, given its scroll
/// `offset` and item `len`. `None` if the cell is on a border or past the list.
fn row_at(pane: Rect, offset: usize, len: usize, col: u16, row: u16) -> Option<usize> {
    if !pane.contains(Position::new(col, row)) {
        return None;
    }
    let top = pane.y + 1; // first content row, below the top border
    if row < top || row >= pane.bottom() - 1 {
        return None; // top / bottom border
    }
    let idx = offset + (row - top) as usize;
    (idx < len).then_some(idx)
}

/// Accent the border of the pane that currently holds keyboard focus.
fn focus_ring(block: Block<'_>, focused: bool) -> Block<'_> {
    if focused {
        block.border_style(Style::new().fg(theme::ACCENT))
    } else {
        block
    }
}

fn main() -> std::io::Result<()> {
    let demo = std::env::args().any(|a| a == "--demo");
    // ponytail: spike — PCI is dump-only for now, no TUI tab yet
    if std::env::args().any(|a| a == "--pci") {
        pci::dump();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--dump") {
        dump(demo);
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
    // mouse capture trades the terminal's native text selection for click/scroll/
    // right-click; we hand back copy via OSC 52 (yank + right-click menu).
    // ponytail: most terminals fall back to shift-drag for native selection
    let _ = ratatui::crossterm::execute!(std::io::stdout(), EnableMouseCapture);
    let result = App::new(demo).run(&mut terminal);
    let _ = ratatui::crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

/// Non-TUI mode: print the tree once and exit.
fn dump(demo: bool) {
    let devices = if demo { usb::demo_scan(0) } else { usb::scan() };
    let rows = usb::flatten(&devices, &HashSet::new());
    let rails = rails(&rows);
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

/// One hot-plug entry: the styled line for display, plus the raw id / name for
/// yanking (`y` = id, `Y` = name).
struct LogEvent {
    line: Line<'static>,
    id: String,
    name: String,
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
enum Focus {
    Tree,
    Events,
}

/// Which device bus the TUI is showing. USB is the full live view; PCI is a
/// read-only bus-grouped tree (`p` toggles).
#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Usb,
    Pci,
}

/// One row of the PCI tree: a "segment:bus" header, or a device (index into
/// `App::pci`).
enum PciRow {
    Bus(String),
    Dev(usize),
}

/// A stable handle to the selected PCI row, snapshotted before a rescan so the
/// selection can be restored by identity (device address / bus name) rather than
/// row index, which shifts as devices come and go.
enum PciSel {
    Dev(String),
    Bus(String),
}

/// Live tree filter (opened with `/`). `editing` = keystrokes go to `query`;
/// once committed (Enter) the filter stays applied while you navigate.
struct Filter {
    query: String,
    editing: bool,
}

/// True if `d` matches the lowercased `q` on any human-facing field.
fn device_matches(d: &Device, q: &str) -> bool {
    d.name.to_lowercase().contains(q)
        || d.label().to_lowercase().contains(q)
        || d.vendor_name().to_lowercase().contains(q)
        || d.class_name().to_lowercase().contains(q)
        || format!("{:04x}:{:04x}", d.vid, d.pid).contains(q)
}

/// Keep every matched row plus its ancestor chain and full subtree, so matches
/// stay anchored in the tree instead of floating parentless.
fn visible_rows(rows: &[(usize, usize)], matched: &[bool]) -> Vec<(usize, usize)> {
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

/// Right-click copy menu. Items are (label, clipboard text, toast noun).
struct ContextMenu {
    rect: Rect,
    items: Vec<(String, String, String)>,
    hover: usize,
}

impl ContextMenu {
    /// Which item (if any) sits under the given screen cell.
    fn item_at(&self, col: u16, row: u16) -> Option<usize> {
        if col <= self.rect.x || col >= self.rect.right() - 1 {
            return None; // outside content columns (borders)
        }
        let i = row.checked_sub(self.rect.y + 1)? as usize;
        (i < self.items.len()).then_some(i)
    }
}

struct App {
    /// scripted fake devices + traffic (`--demo`)
    demo: bool,
    devices: Vec<Device>,
    /// devices + lingering ghosts of removed ones; what the tree shows
    render: Vec<Device>,
    rows: Vec<(usize, usize)>, // (depth, index into render)
    flash: HashMap<String, Instant>,
    ghosts: Vec<(Device, Instant)>,
    collapsed: HashSet<String>,
    list: ListState,
    log: VecDeque<LogEvent>,
    log_state: ListState,
    focus: Focus,
    started: Instant,
    last_scan: Instant,
    metrics: Metrics,
    /// per-device activity history, newest last
    rates: HashMap<String, Vec<u64>>,
    /// transient status line (e.g. "copied …"), shown until it ages out
    toast: Option<(String, Instant)>,
    /// newer release version once the background check finds one (no auto-update)
    update: Option<String>,
    /// one-shot channel carrying the newer version from the check thread
    update_rx: Option<Receiver<String>>,
    /// full frame + pane rects from the last draw, for mapping mouse cells to rows
    screen: Rect,
    tree_rect: Rect,
    log_rect: Rect,
    /// open right-click copy menu, if any
    menu: Option<ContextMenu>,
    /// live tree filter (`/`), if any
    filter: Option<Filter>,
    /// which bus view is shown (USB / PCI)
    tab: Tab,
    /// PCI devices (flat, address-sorted); only refreshed while the PCI tab is up
    pci: Vec<PciDevice>,
    /// computed PCI tree rows (bus headers + devices), collapse/filter applied
    pci_rows: Vec<PciRow>,
    pci_list: ListState,
}

impl App {
    fn new(demo: bool) -> Self {
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
            screen: Rect::default(),
            tree_rect: Rect::default(),
            log_rect: Rect::default(),
            menu: None,
            filter: None,
            tab: Tab::Usb,
            pci: Vec::new(),
            pci_rows: Vec::new(),
            pci_list: ListState::default(),
        }
    }

    fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;
            // ponytail: 1s enumeration poll — switch to nusb::watch_devices()
            // hotplug events if latency matters
            if event::poll(RESCAN_INTERVAL.saturating_sub(self.last_scan.elapsed()))? {
                match event::read()? {
                    // typing into the `/` filter grabs keys before any binding
                    Event::Key(key)
                        if key.kind == KeyEventKind::Press
                            && self.filter.as_ref().is_some_and(|f| f.editing) =>
                    {
                        self.filter_key(key.code)
                    }
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        // any keypress dismisses an open menu; Esc then does nothing else
                        let menu_was_open = self.menu.take().is_some();
                        match key.code {
                            KeyCode::Esc if menu_was_open => {}
                            KeyCode::Esc if self.filter.is_some() => self.clear_filter(),
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char('/') => self.open_filter(),
                            KeyCode::Char('p') => self.toggle_tab(),
                            KeyCode::Tab | KeyCode::BackTab if self.tab == Tab::Usb => {
                                self.toggle_focus()
                            }
                            KeyCode::Down | KeyCode::Char('j') => self.nav(1),
                            KeyCode::Up | KeyCode::Char('k') => self.nav(-1),
                            KeyCode::Char('g') | KeyCode::Home => self.nav_to(0),
                            KeyCode::Char('G') | KeyCode::End => self.nav_to(isize::MAX),
                            KeyCode::Enter | KeyCode::Char(' ') if self.tab == Tab::Usb => {
                                self.fold(None)
                            }
                            KeyCode::Left | KeyCode::Char('h') if self.tab == Tab::Usb => {
                                self.fold(Some(true))
                            }
                            KeyCode::Right | KeyCode::Char('l') if self.tab == Tab::Usb => {
                                self.fold(Some(false))
                            }
                            KeyCode::Char('r') => self.rescan(),
                            KeyCode::Char('y') if self.tab == Tab::Usb => self.yank(false),
                            KeyCode::Char('Y') if self.tab == Tab::Usb => self.yank(true),
                            _ => {}
                        }
                    }
                    Event::Mouse(m) if self.tab == Tab::Usb => self.on_mouse(m),
                    _ => {}
                }
            }
            if self.last_scan.elapsed() >= RESCAN_INTERVAL {
                self.rescan();
            }
            if let Some(rx) = &self.update_rx
                && let Ok(v) = rx.try_recv()
            {
                self.update = Some(v);
                self.update_rx = None;
            }
        }
    }

    /// Move keyboard focus to `f`. Entering the events pane parks the cursor on
    /// the newest entry; leaving it clears the selection so the pane snaps back
    /// to newest.
    fn set_focus(&mut self, f: Focus) {
        self.focus = f;
        match f {
            Focus::Events if self.log_state.selected().is_none() && !self.log.is_empty() => {
                self.log_state.select(Some(0))
            }
            Focus::Tree => self.log_state.select(None),
            _ => {}
        }
    }

    /// Toggle keyboard focus between the tree and events panes.
    fn toggle_focus(&mut self) {
        self.set_focus(match self.focus {
            Focus::Tree => Focus::Events,
            Focus::Events => Focus::Tree,
        });
    }

    /// Open (or re-open) the `/` filter for editing, keeping any prior query.
    fn open_filter(&mut self) {
        self.menu = None;
        self.set_focus(Focus::Tree);
        let query = self.filter.take().map(|f| f.query).unwrap_or_default();
        self.filter = Some(Filter { query, editing: true });
    }

    /// Drop the filter and show the full tree again.
    fn clear_filter(&mut self) {
        self.filter = None;
        self.rebuild_rows();
    }

    /// Switch USB ⇄ PCI. Entering PCI enumerates (or uses demo data) and parks
    /// the selection on the first row.
    fn toggle_tab(&mut self) {
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

    /// Re-enumerate PCI and rebuild its rows, keeping the selection on the same
    /// address if it survived. Snapshot the addr *before* swapping `self.pci` —
    /// the new scan may have a different device count, so the old `pci_rows`
    /// indices no longer line up with the new vec.
    fn pci_rescan(&mut self) {
        let keep = self.pci_selected_key();
        self.pci = if self.demo { pci::demo_scan() } else { pci::scan() };
        self.rebuild_pci_rows_keeping(keep);
    }

    /// Selected PCI device index, if a device row (not a bus header) is selected.
    fn pci_selected(&self) -> Option<usize> {
        match self.pci_rows.get(self.pci_list.selected()?)? {
            PciRow::Dev(i) => Some(*i),
            PciRow::Bus(_) => None,
        }
    }

    /// Identity of the selected row (device address or bus name), for restoring
    /// selection across a rescan. Reads `pci_rows` and `pci` together, so only
    /// valid while the two are in sync (call before swapping `self.pci`).
    fn pci_selected_key(&self) -> Option<PciSel> {
        match self.pci_rows.get(self.pci_list.selected()?)? {
            PciRow::Dev(i) => Some(PciSel::Dev(self.pci[*i].addr.clone())),
            PciRow::Bus(b) => Some(PciSel::Bus(b.clone())),
        }
    }

    /// Group address-sorted devices into bus-headed rows, applying the filter.
    fn compute_pci_rows(&self) -> Vec<PciRow> {
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

    /// Recompute PCI rows (filter applied) and keep the selection on the same
    /// device address if it survived, else clamp to the top. Use when `self.pci`
    /// is unchanged (e.g. filter edits); rescans use `rebuild_pci_rows_keeping`.
    fn rebuild_pci_rows(&mut self) {
        let keep = self.pci_selected_key();
        self.rebuild_pci_rows_keeping(keep);
    }

    /// Rebuild rows and restore the selection to `keep` (snapshotted from the
    /// pre-scan state). If that row is gone, hold the previous index (clamped)
    /// rather than snapping to the top — a bus header has no address, so an
    /// index fallback is what keeps the cursor put across ticks.
    fn rebuild_pci_rows_keeping(&mut self, keep: Option<PciSel>) {
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

    /// Handle a keystroke while typing in the filter box.
    fn filter_key(&mut self, code: KeyCode) {
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

    /// Recompute the displayed rows (collapse + filter) and keep the selection
    /// on the same device if it survived, else clamp to the top.
    fn rebuild_rows(&mut self) {
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

    /// Flatten `render` under the current collapse set, then apply the filter.
    // ponytail: filter only sees uncollapsed rows; expand to search hidden
    // subtrees if that ever bites
    fn compute_rows(&self) -> Vec<(usize, usize)> {
        let rows = usb::flatten(&self.render, &self.collapsed);
        let Some(f) = &self.filter else { return rows };
        let q = f.query.to_lowercase();
        if q.is_empty() {
            return rows;
        }
        let matched: Vec<bool> = rows
            .iter()
            .map(|&(_, i)| device_matches(&self.render[i], &q))
            .collect();
        visible_rows(&rows, &matched)
    }

    /// Route a mouse event: right-click opens the copy menu, left-click selects
    /// or fires a menu item, wheel scrolls the pane under the pointer.
    fn on_mouse(&mut self, m: MouseEvent) {
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

    /// Tree row under a screen cell, accounting for the border and scroll offset.
    fn tree_row_at(&self, col: u16, row: u16) -> Option<usize> {
        row_at(self.tree_rect, self.list.offset(), self.rows.len(), col, row)
    }

    /// Events row under a screen cell.
    fn log_row_at(&self, col: u16, row: u16) -> Option<usize> {
        row_at(self.log_rect, self.log_state.offset(), self.log.len(), col, row)
    }

    /// Left-click: run a menu item if the menu is open (else dismiss it), or
    /// select the clicked row in whichever pane was hit.
    fn click(&mut self, col: u16, row: u16) {
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

    /// Wheel scroll moves the selection in whichever pane the pointer is over.
    fn scroll_at(&mut self, col: u16, row: u16, delta: isize) {
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

    /// Open the right-click copy menu for the row under the cursor, selecting it
    /// first so the detail pane follows. No-op on empty space.
    fn open_menu(&mut self, col: u16, row: u16) {
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

    /// Copy `text` to the clipboard and raise a toast naming `what`.
    fn copy(&mut self, text: &str, what: &str) {
        self.toast = Some((
            match clip(text) {
                Ok(()) => format!("copied {what}"),
                Err(e) => format!("copy failed: {e}"),
            },
            Instant::now(),
        ));
    }

    /// Move the selection in the focused pane. Selection drives ratatui's List
    /// scroll; in the events pane a higher index is an older entry (newest-first).
    fn nav(&mut self, delta: isize) {
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

    /// Jump to an absolute row in the focused pane (`0` = top, `isize::MAX` = bottom).
    fn nav_to(&mut self, idx: isize) {
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

    /// Copy the selected device to the clipboard. `full` grabs a labelled
    /// block; otherwise just the `vid:pid`.
    fn yank(&mut self, full: bool) {
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

    fn rescan(&mut self) {
        self.last_scan = Instant::now();
        if self.tab == Tab::Pci {
            self.pci_rescan();
        }
        let new = if self.demo {
            usb::demo_scan(self.started.elapsed().as_secs())
        } else {
            usb::scan()
        };
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

    /// 0..1 age of a lingering removed device, if this row is one.
    fn ghost_age(&self, name: &str) -> Option<f32> {
        self.ghosts
            .iter()
            .find(|(d, _)| d.name == name)
            .map(|(_, t)| t.elapsed().as_secs_f32() / HIGHLIGHT_TTL.as_secs_f32())
    }

    /// 0..1 age of a freshly plugged device, if this row is one.
    fn flash_age(&self, name: &str) -> Option<f32> {
        self.flash
            .get(name)
            .map(|t| t.elapsed().as_secs_f32() / HIGHLIGHT_TTL.as_secs_f32())
    }

    /// `None` toggles, `Some(true)` folds, `Some(false)` unfolds.
    fn fold(&mut self, want: Option<bool>) {
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

    fn draw(&mut self, f: &mut Frame) {
        if self.tab == Tab::Pci {
            return self.draw_pci(f);
        }
        let [header, main, log_area, help] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(14),
            Constraint::Length(1),
        ])
        .areas(f.area().inner(Margin::new(1, 0)));
        let [tree_area, detail_area] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                .areas(main);
        // stash geometry so mouse cells can be mapped back to rows
        self.screen = f.area();
        self.tree_rect = tree_area;
        self.log_rect = log_area;

        self.draw_header(f, header);
        self.draw_tree(f, tree_area);
        self.draw_detail(f, detail_area);
        self.draw_log(f, log_area);

        // fresh toast takes over the help line for a couple seconds, else key hints
        let showed_toast = if let Some((msg, t)) = &self.toast
            && t.elapsed() < Duration::from_secs(2)
        {
            let ok = msg.starts_with("copied");
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    if ok { " ✓ " } else { " ✗ " }
                        .fg(if ok { theme::MINT } else { theme::ROSE })
                        .bold(),
                    msg.clone().fg(theme::TEXT),
                ])),
                help,
            );
            true
        } else {
            false
        };
        if !showed_toast {
            let keys = [
                ("j/k", "move"),
                ("↵", "toggle"),
                ("h/l", "fold/unfold"),
                ("g/G", "top/bottom"),
                ("/", "filter"),
                ("tab", "focus"),
                ("y/Y", "yank"),
                ("p", "pci"),
                ("r", "rescan"),
                ("q", "quit"),
            ];
            let mut spans = vec![Span::raw(" ")];
            for (key, desc) in keys {
                spans.push(key.fg(theme::ACCENT).bold());
                spans.push(format!(" {desc}   ").fg(theme::DIM));
            }
            f.render_widget(Paragraph::new(Line::from(spans)), help);
        }

        // version pinned bottom-right; upgrade badge when a newer release exists
        let ver = env!("CARGO_PKG_VERSION");
        let right = match &self.update {
            Some(new) => Line::from(vec![
                format!("v{ver} ").fg(theme::DIM),
                format!("↑ v{new} ").fg(theme::MINT).bold(),
            ]),
            None => Line::from(format!("v{ver} ").fg(theme::FAINT)),
        };
        f.render_widget(Paragraph::new(right).alignment(Alignment::Right), help);

        // right-click copy menu floats on top of everything
        if let Some(menu) = &self.menu {
            f.render_widget(Clear, menu.rect);
            let items: Vec<ListItem> = menu
                .items
                .iter()
                .enumerate()
                .map(|(i, (label, _, _))| {
                    let item = ListItem::new(Line::from(format!(" {label}")));
                    if i == menu.hover {
                        item.style(Style::new().bg(theme::SEL_BG).fg(theme::ACCENT))
                    } else {
                        item
                    }
                })
                .collect();
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(theme::ACCENT))
                .title(Line::from(" copy ".fg(theme::ACCENT).bold()));
            f.render_widget(
                List::new(items).style(Style::new().fg(theme::TEXT)).block(block),
                menu.rect,
            );
        }
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let buses = self.devices.iter().filter(|d| d.is_root_hub()).count();
        let up = self.started.elapsed().as_secs();
        let line = Line::from(vec![
            Span::styled(
                " usbtree ",
                Style::new().bg(theme::PILL).fg(theme::PILL_FG).bold(),
            ),
            Span::raw("  "),
            self.devices.len().to_string().fg(theme::TEXT).bold(),
            " devices".fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            buses.to_string().fg(theme::TEXT).bold(),
            " buses".fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            format!("up {:02}:{:02}", up / 60, up % 60).fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            self.metrics.header_note().fg(if self.metrics.is_bytes() {
                theme::MINT
            } else {
                theme::DIM
            }),
        ]);
        f.render_widget(Paragraph::new(line), area);
    }

    fn draw_tree(&mut self, f: &mut Frame, area: Rect) {
        // filter turns the pane title into the live search box + match count
        let block = match &self.filter {
            Some(flt) => {
                let q = flt.query.to_lowercase();
                let n = self
                    .rows
                    .iter()
                    .filter(|&&(_, i)| device_matches(&self.render[i], &q))
                    .count();
                let title = Line::from(vec![
                    " / ".fg(theme::ACCENT).bold(),
                    format!("{}{}", flt.query, if flt.editing { "▏" } else { "" }).fg(theme::TEXT),
                    format!("  {n} match{} ", if n == 1 { "" } else { "es" }).fg(theme::DIM),
                ]);
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(theme::BORDER))
                    .title(title)
                    .padding(Padding::horizontal(1))
            }
            None => pane("✦ tree"),
        };
        let block = focus_ring(block, self.focus == Focus::Tree);
        if self.filter.is_some() && self.rows.is_empty() {
            f.render_widget(
                Paragraph::new("no matches".fg(theme::DIM).italic()).block(block),
                area,
            );
            return;
        }
        let rails = rails(&self.rows);
        let selected = self.list.selected();
        let items: Vec<ListItem> = self
            .rows
            .iter()
            .enumerate()
            .map(|(row, &(_, i))| {
                let d = &self.render[i];
                let ghost = self.ghost_age(&d.name);
                let flash = self.flash_age(&d.name);
                // one fading override color for freshly plugged / unplugged rows
                let fade = |base: Color| match (ghost, flash) {
                    (Some(t), _) => lerp(theme::ROSE, theme::DIM, t),
                    (_, Some(t)) => lerp(theme::MINT, base, t),
                    _ => base,
                };

                let mut spans = vec![
                    if selected == Some(row) {
                        "▌ ".fg(theme::ACCENT)
                    } else {
                        Span::raw("  ")
                    },
                    // fixed-width class gutter: aligned column, easy to scan
                    Span::styled(
                        format!(" {:<8.8} ", d.class_name()),
                        Style::new()
                            .fg(fade(class_color(d.effective_class())))
                            .bg(theme::SURFACE),
                    ),
                    Span::raw(" "),
                    rails[row].clone().fg(theme::FAINT),
                ];
                let kids = usb::child_count(&self.render, &d.name);
                let folded = self.collapsed.contains(&d.name);
                spans.push(if kids == 0 {
                    Span::raw("  ")
                } else if folded {
                    "▸ ".fg(theme::ACCENT).bold()
                } else {
                    "▾ ".fg(theme::FAINT)
                });

                spans.push(format!("{:<8}", d.name).fg(fade(theme::DIM)));
                spans.push(Span::raw(format!(" {} ", d.icon())));
                let label_color = fade(if d.is_root_hub() {
                    theme::ACCENT
                } else {
                    theme::TEXT
                });
                let mut label = format!("{} ", d.label()).fg(label_color);
                if d.is_root_hub() {
                    label = label.bold();
                }
                if ghost.is_some() {
                    label = label.crossed_out();
                }
                spans.push(label);
                if let Some((glyph, human, color)) = speed_badge(&d.speed) {
                    spans.push(format!("  {glyph} {human}").fg(color));
                }
                if folded {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!(" +{kids} "),
                        Style::new().bg(theme::PILL).fg(theme::PILL_FG).bold(),
                    ));
                }

                // right-aligned block, fixed-width columns so they stack tidily:
                // [spark:SPARK_WIDTH] [rate:RATE_W] [badge — only when present]
                const RATE_W: usize = 9;
                let h = self.rates.get(&d.name);
                let has_traffic =
                    h.is_some_and(|h| h.iter().rev().take(SPARK_WIDTH).any(|&v| v > 0));
                let badge = match (ghost.is_some(), flash.is_some()) {
                    (true, _) => Some(("○ unplugged", fade(theme::TEXT))),
                    (_, true) => Some(("● plugged", fade(theme::TEXT))),
                    _ => None,
                };
                if has_traffic || badge.is_some() {
                    let cur = h.and_then(|h| h.last().copied()).unwrap_or(0);
                    let rate = if cur > 0 {
                        fmt_rate(cur, self.metrics.is_bytes())
                    } else {
                        String::new()
                    };
                    // ghost rows keep their frozen old data but tinted red via fade()
                    let metric = fade(theme::MINT);
                    // inner width = area minus border(2) + horizontal padding(2)
                    let inner = area.width.saturating_sub(4) as usize;
                    let left_w: usize = spans.iter().map(Span::width).sum();
                    // responsive: rate always right-aligns; the sparkline is
                    // decoration, dropped when the pane is too narrow to hold it
                    // (full history still lives in the detail pane). Too tight
                    // for the number → a single activity tick.
                    let room = inner.saturating_sub(left_w);
                    let mut right = Vec::new();
                    if has_traffic && room >= SPARK_WIDTH + 1 + RATE_W + 2 {
                        right.push(format!("{:>SPARK_WIDTH$} ", sparkline(h.unwrap(), SPARK_WIDTH)).fg(metric));
                    }
                    if room >= RATE_W + 2 {
                        right.push(format!("{rate:>RATE_W$}").fg(metric).bold());
                    } else if has_traffic {
                        right.push("▪".fg(metric).bold());
                    }
                    if let Some((btext, bcolor)) = badge {
                        right.push(format!("  {btext}").fg(bcolor).bold());
                    }
                    if !right.is_empty() {
                        let right_w: usize = right.iter().map(Span::width).sum();
                        let pad = inner.saturating_sub(left_w + right_w).max(2);
                        spans.push(Span::raw(" ".repeat(pad)));
                        spans.extend(right);
                    }
                }
                ListItem::new(Line::from(spans))
            })
            .collect();
        let list = List::new(items)
            .style(Style::new().fg(theme::TEXT))
            .block(block)
            .scroll_padding(2)
            .highlight_style(Style::new().bg(theme::SEL_BG));
        f.render_stateful_widget(list, area, &mut self.list);
    }

    fn draw_detail(&self, f: &mut Frame, area: Rect) {
        let block = pane("details");
        let Some(&(_, i)) = self.list.selected().and_then(|s| self.rows.get(s)) else {
            f.render_widget(block, area);
            return;
        };
        let d = &self.render[i];
        let key = |k: &str| format!("{k:<10}").fg(theme::DIM);
        let mut lines = vec![
            Line::from(format!("{} {}", d.icon(), d.label()).fg(theme::TEXT).bold()),
            Line::from(d.vendor_name().fg(theme::DIM)),
            Line::from("─".repeat(24).fg(theme::FAINT)),
            Line::from(vec![key("sysfs"), d.name.clone().fg(theme::TEXT)]),
            Line::from(vec![
                key("vid:pid"),
                format!("{:04x}:{:04x}", d.vid, d.pid).fg(theme::ACCENT),
            ]),
            Line::from(vec![
                key("class"),
                d.class_name().fg(class_color(d.effective_class())),
                // device-level triple bDeviceClass:SubClass:Protocol (name is the
                // friendly effective class; the code is the raw device descriptor)
                format!("  {:02x}:{:02x}:{:02x}", d.class, d.subclass, d.protocol)
                    .fg(theme::FAINT),
            ]),
        ];
        if d.usb_version != 0 {
            lines.push(Line::from(vec![
                key("usb"),
                bcd(d.usb_version).fg(theme::TEXT),
                "  spec".fg(theme::FAINT),
            ]));
        }
        if d.device_version != 0 {
            lines.push(Line::from(vec![
                key("rev"),
                bcd(d.device_version).fg(theme::TEXT),
            ]));
        }
        if let Some((glyph, human, color)) = speed_badge(&d.speed) {
            lines.push(Line::from(vec![
                key("speed"),
                format!("{glyph} {human}").fg(color),
                format!("  {} Mbps", d.speed).fg(theme::FAINT),
            ]));
        }
        if let Some(ma) = d.max_power_ma {
            lines.push(Line::from(vec![
                key("power"),
                format!("{ma} mA").fg(theme::TEXT),
                "  max".fg(theme::FAINT),
            ]));
        }
        if let Some(a) = d.config_attributes {
            let mut spans = vec![
                key("powered"),
                if a & 0x40 != 0 { "self" } else { "bus" }.fg(theme::TEXT),
            ];
            if a & 0x20 != 0 {
                spans.push("  remote-wakeup".fg(theme::FAINT));
            }
            lines.push(Line::from(spans));
        }
        if let Some(s) = &d.serial {
            lines.push(Line::from(vec![key("serial"), s.clone().fg(theme::TEXT)]));
        }
        let kids = usb::child_count(&self.render, &d.name);
        if kids > 0 {
            lines.push(Line::from(vec![
                key("connected"),
                kids.to_string().fg(theme::TEXT),
            ]));
        }
        if !d.interfaces.is_empty() {
            lines.push(Line::from("─".repeat(24).fg(theme::FAINT)));
            lines.push(Line::from("interfaces".fg(theme::DIM)));
            for i in &d.interfaces {
                let mut head = vec![
                    format!("{:>2} ", i.number).fg(theme::FAINT),
                    usb::class_name(i.class).fg(class_color(i.class)),
                ];
                if let Some(n) = &i.name {
                    head.push(format!("  {n}").fg(theme::TEXT));
                }
                head.push(
                    format!("  {:02x}:{:02x}:{:02x}", i.class, i.subclass, i.protocol)
                        .fg(theme::FAINT),
                );
                if i.alt != 0 {
                    head.push(format!("  alt{}", i.alt).fg(theme::FAINT));
                }
                lines.push(Line::from(head));
                for e in &i.endpoints {
                    lines.push(Line::from(vec![
                        format!("    {:#04x} ", e.address).fg(theme::DIM),
                        if e.input { "IN  " } else { "OUT " }.fg(theme::ACCENT),
                        format!("{:<4} ", ep_type(e.transfer)).fg(theme::TEXT),
                        format!("{}B", e.max_packet).fg(theme::DIM),
                        match e.transfer {
                            1 | 3 if e.interval != 0 => format!("  @{}", e.interval),
                            _ => String::new(),
                        }
                        .fg(theme::FAINT),
                    ]));
                }
            }
        }
        let inner = block.inner(area);
        f.render_widget(block, area);
        let [kv, spark] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(5)]).areas(inner);
        f.render_widget(Paragraph::new(lines), kv);
        if let Some(h) = self.rates.get(&d.name) {
            let bytes = self.metrics.is_bytes();
            let cur = h.last().copied().unwrap_or(0);
            let [title, graph] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(spark);
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    if bytes { "bandwidth " } else { "activity " }.fg(theme::DIM),
                    fmt_rate(cur, bytes).fg(theme::MINT).bold(),
                    if bytes { "" } else { " URBs" }.fg(theme::FAINT),
                ])),
                title,
            );
            f.render_widget(
                Sparkline::default()
                    .data(h)
                    .style(Style::new().fg(theme::MINT)),
                graph,
            );
        }
    }

    fn draw_log(&mut self, f: &mut Frame, area: Rect) {
        let focused = self.focus == Focus::Events;
        let block = focus_ring(pane("events"), focused);
        if self.log.is_empty() {
            f.render_widget(
                Paragraph::new(Line::from(
                    "waiting for hot-plug events…".fg(theme::DIM).italic(),
                ))
                .block(block),
                area,
            );
            return;
        }
        // newest entries bright, older ones dim out
        let items = self.log.iter().enumerate().map(|(i, ev)| {
            let item = ListItem::new(ev.line.clone());
            if i >= 4 {
                item.style(Style::new().add_modifier(Modifier::DIM))
            } else {
                item
            }
        });
        let mut list = List::new(items.collect::<Vec<_>>()).block(block);
        if focused {
            list = list.highlight_style(Style::new().bg(theme::SEL_BG));
        }
        f.render_stateful_widget(list, area, &mut self.log_state);
    }

    /// PCI tab: a read-only bus-grouped tree + detail. No events/metrics pane —
    /// PCI has no per-device traffic or hot-plug worth showing.
    fn draw_pci(&mut self, f: &mut Frame) {
        let [header, main, help] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .areas(f.area().inner(Margin::new(1, 0)));
        let [tree_area, detail_area] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                .areas(main);
        self.screen = f.area();
        self.tree_rect = tree_area;

        self.draw_pci_header(f, header);
        self.draw_pci_tree(f, tree_area);
        self.draw_pci_detail(f, detail_area);

        let keys = [
            ("j/k", "move"),
            ("g/G", "top/bottom"),
            ("/", "filter"),
            ("p", "usb view"),
            ("r", "rescan"),
            ("q", "quit"),
        ];
        let mut spans = vec![Span::raw(" ")];
        for (key, desc) in keys {
            spans.push(key.fg(theme::ACCENT).bold());
            spans.push(format!(" {desc}   ").fg(theme::DIM));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), help);
        let ver = env!("CARGO_PKG_VERSION");
        f.render_widget(
            Paragraph::new(Line::from(format!("v{ver} ").fg(theme::FAINT)))
                .alignment(Alignment::Right),
            help,
        );
    }

    fn draw_pci_header(&self, f: &mut Frame, area: Rect) {
        let buses = self
            .pci_rows
            .iter()
            .filter(|r| matches!(r, PciRow::Bus(_)))
            .count();
        let line = Line::from(vec![
            Span::styled(
                " usbtree ",
                Style::new().bg(theme::PILL).fg(theme::PILL_FG).bold(),
            ),
            "  ".into(),
            "usb".fg(theme::DIM),
            " · ".fg(theme::FAINT),
            "pci".fg(theme::ACCENT).bold(),
            "  ·  ".fg(theme::FAINT),
            self.pci.len().to_string().fg(theme::TEXT).bold(),
            " devices".fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            buses.to_string().fg(theme::TEXT).bold(),
            " buses".fg(theme::DIM),
        ]);
        f.render_widget(Paragraph::new(line), area);
    }

    fn draw_pci_tree(&mut self, f: &mut Frame, area: Rect) {
        let block = match &self.filter {
            Some(flt) => {
                let title = Line::from(vec![
                    " / ".fg(theme::ACCENT).bold(),
                    format!("{}{}", flt.query, if flt.editing { "▏" } else { "" }).fg(theme::TEXT),
                ]);
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(theme::ACCENT))
                    .title(title)
                    .padding(Padding::horizontal(1))
            }
            None => pane("✦ pci"),
        };
        if self.pci_rows.is_empty() {
            let msg = if self.filter.is_some() {
                "no matches"
            } else {
                "no PCI devices (backend may be unsupported here)"
            };
            f.render_widget(Paragraph::new(msg.fg(theme::DIM).italic()).block(block), area);
            return;
        }
        // rails only read the depth: bus header = 0, device = 1
        let depths: Vec<(usize, usize)> = self
            .pci_rows
            .iter()
            .map(|r| (matches!(r, PciRow::Dev(_)) as usize, 0))
            .collect();
        let rails = rails(&depths);
        let selected = self.pci_list.selected();
        let items: Vec<ListItem> = self
            .pci_rows
            .iter()
            .enumerate()
            .map(|(row, r)| {
                let sel = if selected == Some(row) {
                    "▌ ".fg(theme::ACCENT)
                } else {
                    Span::raw("  ")
                };
                match r {
                    PciRow::Bus(bus) => ListItem::new(Line::from(vec![
                        sel,
                        format!(" {bus} ").fg(theme::ACCENT).bold(),
                    ])),
                    PciRow::Dev(i) => {
                        let d = &self.pci[*i];
                        ListItem::new(Line::from(vec![
                            sel,
                            Span::styled(
                                format!(" {:<10.10} ", d.class_group()),
                                Style::new().fg(pci_class_color(d.class)).bg(theme::SURFACE),
                            ),
                            Span::raw(" "),
                            rails[row].clone().fg(theme::FAINT),
                            format!("{} ", pci_icon(d.class)).into(),
                            format!("{} ", d.label()).fg(theme::TEXT),
                            d.addr.clone().fg(theme::FAINT),
                        ]))
                    }
                }
            })
            .collect();
        let list = List::new(items)
            .style(Style::new().fg(theme::TEXT))
            .block(block)
            .scroll_padding(2)
            .highlight_style(Style::new().bg(theme::SEL_BG));
        f.render_stateful_widget(list, area, &mut self.pci_list);
    }

    fn draw_pci_detail(&self, f: &mut Frame, area: Rect) {
        let block = pane("details");
        let Some(i) = self.pci_selected() else {
            f.render_widget(block, area);
            return;
        };
        let d = &self.pci[i];
        let key = |k: &str| format!("{k:<10}").fg(theme::DIM);
        let mut lines = vec![
            Line::from(format!("{} {}", pci_icon(d.class), d.label()).fg(theme::TEXT).bold()),
            Line::from(d.vendor_name().fg(theme::DIM)),
            Line::from("─".repeat(24).fg(theme::FAINT)),
            Line::from(vec![key("address"), d.addr.clone().fg(theme::TEXT)]),
            Line::from(vec![
                key("vid:pid"),
                format!("{:04x}:{:04x}", d.vid, d.pid).fg(theme::ACCENT),
            ]),
            Line::from(vec![
                key("class"),
                d.class_name().fg(pci_class_color(d.class)),
                format!("  {:02x}:{:02x}:{:02x}", d.class, d.subclass, d.prog_if).fg(theme::FAINT),
            ]),
        ];
        if let Some(pif) = d.prog_if_name() {
            lines.push(Line::from(vec![key("interface"), pif.fg(theme::TEXT)]));
        }
        lines.push(Line::from(vec![
            key("revision"),
            format!("{:02x}", d.revision).fg(theme::TEXT),
        ]));
        if let Some((v, p)) = d.subsystem {
            let val = d.subsystem_name().unwrap_or_else(|| format!("{v:04x}:{p:04x}"));
            lines.push(Line::from(vec![key("subsystem"), val.fg(theme::TEXT)]));
        }
        if let Some(l) = &d.link {
            let mut spans = vec![
                key("link"),
                format!("x{} {}", l.cur_width, l.cur_speed).fg(theme::TEXT),
            ];
            if l.throttled() {
                spans.push(format!("  ↓max x{} {}", l.max_width, l.max_speed).fg(theme::PEACH));
            }
            lines.push(Line::from(spans));
        }
        if let Some(drv) = &d.driver {
            lines.push(Line::from(vec![key("driver"), drv.clone().fg(theme::TEXT)]));
        }
        if let Some(n) = d.numa_node {
            lines.push(Line::from(vec![key("numa"), n.to_string().fg(theme::TEXT)]));
        }
        if let Some(g) = &d.iommu_group {
            lines.push(Line::from(vec![key("iommu"), g.clone().fg(theme::TEXT)]));
        }
        if let Some(ps) = &d.power_state {
            lines.push(Line::from(vec![key("power"), ps.clone().fg(theme::TEXT)]));
        }
        f.render_widget(Paragraph::new(lines).block(block), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_hit_test_excludes_borders() {
        let m = ContextMenu {
            rect: Rect::new(5, 5, 12, 5), // 3 content rows: y = 6,7,8
            items: vec![(String::new(), String::new(), String::new()); 3],
            hover: 0,
        };
        assert_eq!(m.item_at(6, 6), Some(0)); // first content row
        assert_eq!(m.item_at(6, 8), Some(2)); // last content row
        assert_eq!(m.item_at(6, 5), None); // top border
        assert_eq!(m.item_at(6, 9), None); // bottom border
        assert_eq!(m.item_at(5, 7), None); // left border
        assert_eq!(m.item_at(16, 7), None); // right border (x+width-1)
    }

    #[test]
    fn filter_keeps_ancestors_and_subtree() {
        // usb1(0) > 1-1(1), 1-2(1) > 1-2.1(2), 1-2.2(2)
        let rows = vec![(0, 0), (1, 1), (1, 2), (2, 3), (2, 4)];
        // match a leaf: keep it + its ancestor chain, nothing else
        let m = vec![false, false, false, true, false];
        assert_eq!(visible_rows(&rows, &m), vec![(0, 0), (1, 2), (2, 3)]);
        // match a hub: keep ancestor + whole subtree
        let m = vec![false, false, true, false, false];
        assert_eq!(visible_rows(&rows, &m), vec![(0, 0), (1, 2), (2, 3), (2, 4)]);
        // no match: empty
        assert!(visible_rows(&rows, &[false; 5]).is_empty());
    }

    #[test]
    fn base64_matches_rfc_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(b"046d:c52b"), "MDQ2ZDpjNTJi");
    }
}
