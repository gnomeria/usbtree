mod metrics;
mod usb;

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, List, ListItem, ListState, Padding, Paragraph, Sparkline,
};
use ratatui::Frame;

use metrics::Metrics;
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

/// (tier glyph, human-readable speed, color) — brighter with tier.
fn speed_badge(speed: &str) -> Option<(&'static str, String, Color)> {
    let mbps: f32 = speed.parse().ok()?;
    let (glyph, color) = if mbps >= 5000.0 {
        ("█", theme::ACCENT)
    } else if mbps >= 480.0 {
        ("▄", theme::DIM)
    } else {
        ("▂", theme::FAINT)
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

fn main() -> std::io::Result<()> {
    let demo = std::env::args().any(|a| a == "--demo");
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
    let result = App::new(demo).run(&mut terminal);
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

fn event_line(stamp: &str, added: bool, d: &Device) -> Line<'static> {
    let (glyph, color) = if added {
        ("▲ ", theme::MINT)
    } else {
        ("▼ ", theme::ROSE)
    };
    Line::from(vec![
        stamp.to_string().fg(theme::DIM),
        Span::styled(glyph, Style::new().fg(color).bold()),
        format!("{:<8}", d.name).fg(theme::DIM),
        format!(" {} {}", d.icon(), d.label()).fg(theme::TEXT),
        format!("  {:04x}:{:04x}", d.vid, d.pid).fg(theme::FAINT),
    ])
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
    log: VecDeque<Line<'static>>,
    started: Instant,
    last_scan: Instant,
    metrics: Metrics,
    /// per-device activity history, newest last
    rates: HashMap<String, Vec<u64>>,
}

impl App {
    fn new(demo: bool) -> Self {
        let devices = if demo { usb::demo_scan(0) } else { usb::scan() };
        let rows = usb::flatten(&devices, &HashSet::new());
        let mut list = ListState::default();
        if !rows.is_empty() {
            list.select(Some(0));
        }
        let mut metrics = if demo { Metrics::demo() } else { Metrics::new() };
        metrics.sample(&devices); // baseline so the first tick is a delta, not a total
        Self {
            demo,
            render: devices.clone(),
            devices,
            rows,
            flash: HashMap::new(),
            ghosts: Vec::new(),
            collapsed: HashSet::new(),
            list,
            log: VecDeque::new(),
            started: Instant::now(),
            last_scan: Instant::now(),
            metrics,
            rates: HashMap::new(),
        }
    }

    fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;
            // ponytail: 1s enumeration poll — switch to nusb::watch_devices()
            // hotplug events if latency matters
            if event::poll(RESCAN_INTERVAL.saturating_sub(self.last_scan.elapsed()))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
                    KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
                    KeyCode::Char('g') | KeyCode::Home => self.list.select(Some(0)),
                    KeyCode::Char('G') | KeyCode::End => {
                        self.list.select(Some(self.rows.len().saturating_sub(1)))
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => self.fold(None),
                    KeyCode::Left | KeyCode::Char('h') => self.fold(Some(true)),
                    KeyCode::Right | KeyCode::Char('l') => self.fold(Some(false)),
                    KeyCode::Char('r') => self.rescan(),
                    _ => {}
                }
            }
            if self.last_scan.elapsed() >= RESCAN_INTERVAL {
                self.rescan();
            }
        }
    }

    fn move_sel(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let cur = self.list.selected().unwrap_or(0) as isize;
        let new = (cur + delta).clamp(0, self.rows.len() as isize - 1);
        self.list.select(Some(new as usize));
    }

    fn rescan(&mut self) {
        self.last_scan = Instant::now();
        let new = if self.demo {
            usb::demo_scan(self.started.elapsed().as_secs())
        } else {
            usb::scan()
        };
        let (added, removed) = usb::diff(&self.devices, &new);
        let stamp = self.started.elapsed().as_secs();
        let stamp = format!("[{:02}:{:02}] ", stamp / 60, stamp % 60);
        for d in &added {
            self.log.push_front(event_line(&stamp, true, d));
        }
        for d in &removed {
            self.log.push_front(event_line(&stamp, false, d));
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
        self.rates.retain(|k, _| self.devices.iter().any(|d| &d.name == k));

        // keep selection on the same device across rescans
        let selected_name = self
            .list
            .selected()
            .and_then(|s| self.rows.get(s))
            .map(|&(_, i)| self.render[i].name.clone());
        self.render = self.devices.clone();
        self.render.extend(self.ghosts.iter().map(|(d, _)| d.clone()));
        self.rows = usb::flatten(&self.render, &self.collapsed);
        let sel = selected_name
            .and_then(|n| self.rows.iter().position(|&(_, i)| self.render[i].name == n))
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
        self.rows = usb::flatten(&self.render, &self.collapsed);
        if let Some(pos) = self.rows.iter().position(|&(_, j)| self.render[j].name == name) {
            self.list.select(Some(pos));
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let [header, main, log_area, help] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(8),
            Constraint::Length(1),
        ])
        .areas(f.area().inner(Margin::new(1, 0)));
        let [tree_area, detail_area] =
            Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                .areas(main);

        self.draw_header(f, header);
        self.draw_tree(f, tree_area);
        self.draw_detail(f, detail_area);
        self.draw_log(f, log_area);

        let keys = [
            ("j/k", "move"),
            ("↵", "toggle"),
            ("h/l", "fold/unfold"),
            ("g/G", "top/bottom"),
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

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let buses = self.devices.iter().filter(|d| d.is_root_hub()).count();
        let up = self.started.elapsed().as_secs();
        let line = Line::from(vec![
            Span::styled(" usbtree ", Style::new().bg(theme::PILL).fg(theme::PILL_FG).bold()),
            Span::raw("  "),
            self.devices.len().to_string().fg(theme::TEXT).bold(),
            " devices".fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            buses.to_string().fg(theme::TEXT).bold(),
            " buses".fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            format!("up {:02}:{:02}", up / 60, up % 60).fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            if self.metrics.is_bytes() {
                "◉ usbmon bytes/s".fg(theme::MINT)
            } else {
                "◌ urb activity — sudo for bytes/s".fg(theme::DIM)
            },
        ]);
        f.render_widget(Paragraph::new(line), area);
    }

    fn draw_tree(&mut self, f: &mut Frame, area: Rect) {
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
                        Style::new().fg(fade(class_color(d.effective_class()))).bg(theme::SURFACE),
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
                let label_color = fade(if d.is_root_hub() { theme::ACCENT } else { theme::TEXT });
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
                if let Some(h) = self.rates.get(&d.name)
                    && h.iter().rev().take(SPARK_WIDTH).any(|&v| v > 0)
                {
                    spans.push(format!("  {}", sparkline(h, SPARK_WIDTH)).fg(theme::MINT));
                    let cur = h.last().copied().unwrap_or(0);
                    if cur > 0 {
                        spans.push(
                            format!(" {}", fmt_rate(cur, self.metrics.is_bytes()))
                                .fg(theme::MINT)
                                .bold(),
                        );
                    }
                }
                if folded {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!(" +{kids} "),
                        Style::new().bg(theme::PILL).fg(theme::PILL_FG).bold(),
                    ));
                }
                if ghost.is_some() {
                    spans.push("  ○ unplugged".fg(fade(theme::TEXT)).bold());
                } else if flash.is_some() {
                    spans.push("  ● plugged".fg(fade(theme::TEXT)).bold());
                }
                ListItem::new(Line::from(spans))
            })
            .collect();
        let list = List::new(items)
            .style(Style::new().fg(theme::TEXT))
            .block(pane("✦ tree"))
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
                format!("  0x{:02x}", d.effective_class()).fg(theme::FAINT),
            ]),
        ];
        if let Some((glyph, human, color)) = speed_badge(&d.speed) {
            lines.push(Line::from(vec![
                key("speed"),
                format!("{glyph} {human}").fg(color),
                format!("  {} Mbps", d.speed).fg(theme::FAINT),
            ]));
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
                Sparkline::default().data(h).style(Style::new().fg(theme::MINT)),
                graph,
            );
        }
    }

    fn draw_log(&self, f: &mut Frame, area: Rect) {
        let block = pane("events");
        if self.log.is_empty() {
            f.render_widget(
                Paragraph::new(Line::from("waiting for hot-plug events…".fg(theme::DIM).italic()))
                    .block(block),
                area,
            );
            return;
        }
        // newest entries bright, older ones dim out
        let items = self.log.iter().enumerate().map(|(i, l)| {
            let item = ListItem::new(l.clone());
            if i >= 4 {
                item.style(Style::new().add_modifier(Modifier::DIM))
            } else {
                item
            }
        });
        f.render_widget(List::new(items.collect::<Vec<_>>()).block(block), area);
    }
}
