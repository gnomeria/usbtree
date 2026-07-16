use crate::app::device_matches;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Padding, Paragraph, Sparkline};

use crate::app::{App, Focus, PciRow, Tab};
use crate::usb;

pub mod theme {
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
pub fn class_color(class: u8) -> Color {
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
pub fn pci_class_color(class: u8) -> Color {
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

pub fn pci_icon(class: u8) -> &'static str {
    match class {
        0x01 => "💾",
        0x02 => "🌐",
        0x03 => "🖥",
        0x04 => "🎬",
        0x06 => "🌉",
        0x07 => "📞",
        0x0c => "🔌",
        0x0d => "📶",
        _ => "🔹",
    }
}

/// (tier glyph, human-readable speed, color) — brighter with tier.
pub fn speed_badge(speed: &str) -> Option<(&'static str, String, Color)> {
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
pub const HISTORY: usize = 60;
/// Sparkline width in tree rows.
pub const SPARK_WIDTH: usize = 10;

const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Mini text sparkline of the last `width` samples, scaled to their max.
pub fn sparkline(samples: &[u64], width: usize) -> String {
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

pub fn fmt_rate(v: u64, bytes: bool) -> String {
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
pub fn ep_type(t: u8) -> &'static str {
    ["ctrl", "iso", "bulk", "int"][(t & 0x03) as usize]
}

/// BCD version word (bcdUSB / bcdDevice) as a dotted string: 0x0200 -> "2.00".
pub fn bcd(v: u16) -> String {
    format!("{:x}.{:02x}", v >> 8, v & 0xff)
}

/// Standard base64, no padding elided. ~15 lines beats an extra crate.
pub fn base64(input: &[u8]) -> String {
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
pub fn clip(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = std::io::stdout();
    write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes()))?;
    out.flush()
}

pub fn lerp(a: Color, b: Color, t: f32) -> Color {
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (a, b) else {
        return a;
    };
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
}

/// Tree rail prefix ("│  ├─ └─ ") per row, from the depth sequence.
// ponytail: O(n²) lookahead for last-sibling; fine at USB tree sizes
pub fn rails(rows: &[(usize, usize)]) -> Vec<String> {
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

pub fn pane(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme::BORDER))
        .title(Line::from(format!(" {title} ").fg(theme::ACCENT).bold()))
        .padding(Padding::horizontal(1))
}

/// Map a screen cell to a list row inside a bordered pane, given its scroll
/// `offset` and item `len`. `None` if the cell is on a border or past the list.
pub fn row_at(pane: Rect, offset: usize, len: usize, col: u16, row: u16) -> Option<usize> {
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

/// A `w`×`h` rect centered within `area` (clamped to fit).
pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w.min(area.width),
        height: h.min(area.height),
    }
}

/// Accent the border of the pane that currently holds keyboard focus.
pub fn focus_ring(block: Block<'_>, focused: bool) -> Block<'_> {
    if focused {
        block.border_style(Style::new().fg(theme::ACCENT))
    } else {
        block
    }
}


    pub fn draw(app: &mut App, f: &mut Frame) {
        if app.tab == Tab::Pci {
            return crate::ui::draw_pci(app, f);
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
        app.screen = f.area();
        app.tree_rect = tree_area;
        app.log_rect = log_area;

        crate::ui::draw_header(app, f, header);
        crate::ui::draw_tree(app, f, tree_area);
        crate::ui::draw_detail(app, f, detail_area);
        crate::ui::draw_log(app, f, log_area);

        // an in-flight eject owns the help line until it finishes (may be seconds);
        // else a fresh toast for a couple seconds; else the key hints
        let showed_toast = if let Some(label) = &app.ejecting {
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    " ⏳ ".fg(theme::ACCENT).bold(),
                    format!("ejecting {label}…").fg(theme::TEXT),
                ])),
                help,
            );
            true
        } else if let Some((msg, t, ok)) = &app.toast
            && t.elapsed() < Duration::from_secs(2)
        {
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    if *ok { " ✓ " } else { " ✗ " }
                        .fg(if *ok { theme::MINT } else { theme::ROSE })
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
            let mut keys = vec![
                ("j/k", "move"),
                ("↵", "toggle"),
                ("h/l", "fold/unfold"),
                ("g/G", "top/bottom"),
                ("/", "filter"),
                ("tab", "focus"),
                ("y/Y", "yank"),
            ];
            if app.can_eject() {
                keys.push(("e", "eject"));
            }
            keys.extend([("p", "pci"), ("r", "rescan"), ("q", "quit")]);
            let mut spans = vec![Span::raw(" ")];
            for (key, desc) in keys {
                spans.push(key.fg(theme::ACCENT).bold());
                spans.push(format!(" {desc}   ").fg(theme::DIM));
            }
            f.render_widget(Paragraph::new(Line::from(spans)), help);
        }

        // version pinned bottom-right; upgrade badge when a newer release exists
        let ver = env!("CARGO_PKG_VERSION");
        let right = match &app.update {
            Some(new) => Line::from(vec![
                format!("v{ver} ").fg(theme::DIM),
                format!("↑ v{new} ").fg(theme::MINT).bold(),
            ]),
            None => Line::from(format!("v{ver} ").fg(theme::FAINT)),
        };
        f.render_widget(Paragraph::new(right).alignment(Alignment::Right), help);

        // right-click copy menu floats on top of everything
        if let Some(menu) = &app.menu {
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

        // eject confirmation dialog floats centered above everything
        if let Some(name) = &app.confirm {
            let label = app
                .render
                .iter()
                .find(|d| &d.name == name)
                .map(|d| format!("{} {}", d.icon(), d.label()))
                .unwrap_or_else(|| name.clone());
            let body = vec![
                Line::from(format!("Safely eject {name}?").fg(theme::TEXT).bold()),
                Line::from(label.fg(theme::DIM)),
                Line::from(""),
                Line::from(vec![
                    " unmounts and powers off the drive".fg(theme::FAINT),
                ]),
                Line::from(""),
                Line::from(vec![
                    " e ".fg(theme::PILL_FG).bg(theme::PILL).bold(),
                    " confirm    ".fg(theme::DIM),
                    " esc ".fg(theme::TEXT).bg(theme::SURFACE),
                    " cancel".fg(theme::DIM),
                ]),
            ];
            let w = body.iter().map(Line::width).max().unwrap_or(30) as u16 + 4;
            let rect = centered(app.screen, w, body.len() as u16 + 2);
            f.render_widget(Clear, rect);
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(theme::ROSE))
                .title(Line::from(" eject ".fg(theme::ROSE).bold()))
                .padding(Padding::horizontal(1));
            f.render_widget(Paragraph::new(body).block(block), rect);
        }
    }

    pub fn draw_header(app: &App, f: &mut Frame, area: Rect) {
        let buses = app.devices.iter().filter(|d| d.is_root_hub()).count();
        let up = app.started.elapsed().as_secs();
        let line = Line::from(vec![
            Span::styled(
                " usbtree ",
                Style::new().bg(theme::PILL).fg(theme::PILL_FG).bold(),
            ),
            Span::raw("  "),
            app.devices.len().to_string().fg(theme::TEXT).bold(),
            " devices".fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            buses.to_string().fg(theme::TEXT).bold(),
            " buses".fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            format!("up {:02}:{:02}", up / 60, up % 60).fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            app.metrics.header_note().fg(if app.metrics.is_bytes() {
                theme::MINT
            } else {
                theme::DIM
            }),
        ]);
        f.render_widget(Paragraph::new(line), area);
    }

    pub fn draw_tree(app: &mut App, f: &mut Frame, area: Rect) {
        // filter turns the pane title into the live search box + match count
        let block = match &app.filter {
            Some(flt) => {
                let q = flt.query.to_lowercase();
                let n = app
                    .rows
                    .iter()
                    .filter(|&&(_, i)| device_matches(&app.render[i], &q))
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
        let block = focus_ring(block, app.focus == Focus::Tree);
        if app.filter.is_some() && app.rows.is_empty() {
            f.render_widget(
                Paragraph::new("no matches".fg(theme::DIM).italic()).block(block),
                area,
            );
            return;
        }
        let rails = rails(&app.rows);
        let selected = app.list.selected();
        let items: Vec<ListItem> = app
            .rows
            .iter()
            .enumerate()
            .map(|(row, &(_, i))| {
                let d = &app.render[i];
                let ghost = app.ghost_age(&d.name);
                let flash = app.flash_age(&d.name);
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
                let kids = usb::child_count(&app.render, &d.name);
                let folded = app.collapsed.contains(&d.name);
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
                let h = app.rates.get(&d.name);
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
                        fmt_rate(cur, app.metrics.is_bytes())
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
        f.render_stateful_widget(list, area, &mut app.list);
    }

    pub fn draw_detail(app: &App, f: &mut Frame, area: Rect) {
        let block = pane("details");
        let Some(&(_, i)) = app.list.selected().and_then(|s| app.rows.get(s)) else {
            f.render_widget(block, area);
            return;
        };
        let d = &app.render[i];
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
                if a & 0x40 != 0 { "app" } else { "bus" }.fg(theme::TEXT),
            ];
            if a & 0x20 != 0 {
                spans.push("  remote-wakeup".fg(theme::FAINT));
            }
            lines.push(Line::from(spans));
        }
        if let Some(s) = &d.serial {
            lines.push(Line::from(vec![key("serial"), s.clone().fg(theme::TEXT)]));
        }
        let kids = usb::child_count(&app.render, &d.name);
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
        if let Some(h) = app.rates.get(&d.name) {
            let bytes = app.metrics.is_bytes();
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

    pub fn draw_log(app: &mut App, f: &mut Frame, area: Rect) {
        let focused = app.focus == Focus::Events;
        let block = focus_ring(pane("events"), focused);
        if app.log.is_empty() {
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
        let items = app.log.iter().enumerate().map(|(i, ev)| {
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
        f.render_stateful_widget(list, area, &mut app.log_state);
    }

    pub fn draw_pci(app: &mut App, f: &mut Frame) {
        let [header, main, help] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .areas(f.area().inner(Margin::new(1, 0)));
        let [tree_area, detail_area] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                .areas(main);
        app.screen = f.area();
        app.tree_rect = tree_area;

        crate::ui::draw_pci_header(app, f, header);
        crate::ui::draw_pci_tree(app, f, tree_area);
        crate::ui::draw_pci_detail(app, f, detail_area);

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

    pub fn draw_pci_header(app: &App, f: &mut Frame, area: Rect) {
        let buses = app
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
            app.pci.len().to_string().fg(theme::TEXT).bold(),
            " devices".fg(theme::DIM),
            "  ·  ".fg(theme::FAINT),
            buses.to_string().fg(theme::TEXT).bold(),
            " buses".fg(theme::DIM),
        ]);
        f.render_widget(Paragraph::new(line), area);
    }

    pub fn draw_pci_tree(app: &mut App, f: &mut Frame, area: Rect) {
        let block = match &app.filter {
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
        if app.pci_rows.is_empty() {
            let msg = if app.filter.is_some() {
                "no matches"
            } else {
                "no PCI devices (backend may be unsupported here)"
            };
            f.render_widget(Paragraph::new(msg.fg(theme::DIM).italic()).block(block), area);
            return;
        }
        // rails only read the depth: bus header = 0, device = 1
        let depths: Vec<(usize, usize)> = app
            .pci_rows
            .iter()
            .map(|r| (matches!(r, PciRow::Dev(_)) as usize, 0))
            .collect();
        let rails = rails(&depths);
        let selected = app.pci_list.selected();
        let items: Vec<ListItem> = app
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
                        let d = &app.pci[*i];
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
        f.render_stateful_widget(list, area, &mut app.pci_list);
    }

    pub fn draw_pci_detail(app: &App, f: &mut Frame, area: Rect) {
        let block = pane("details");
        let Some(i) = app.pci_selected() else {
            f.render_widget(block, area);
            return;
        };
        let d = &app.pci[i];
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
