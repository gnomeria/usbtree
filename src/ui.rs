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

    struct ThemeColors {
        accent: Color,
        pill: Color,
        pill_fg: Color,
        text: Color,
        dim: Color,
        faint: Color,
        border: Color,
        surface: Color,
        sel_bg: Color,
        mint: Color,
        rose: Color,
        blue: Color,
        teal: Color,
        yellow: Color,
        peach: Color,
        mauve: Color,
        green: Color,
        sky: Color,
    }

    const MOCHA: ThemeColors = ThemeColors {
        accent: Color::Rgb(0xb4, 0x8e, 0xff), // lavender
        pill: Color::Rgb(0x7d, 0x56, 0xf4),   // charm purple
        pill_fg: Color::Rgb(0xf8, 0xf8, 0xfc),
        text: Color::Rgb(0xcd, 0xd6, 0xf4),
        dim: Color::Rgb(0x6c, 0x70, 0x86),
        faint: Color::Rgb(0x45, 0x47, 0x5a),
        border: Color::Rgb(0x36, 0x38, 0x4e),
        surface: Color::Rgb(0x2e, 0x30, 0x45),
        sel_bg: Color::Rgb(0x2a, 0x2c, 0x40),
        mint: Color::Rgb(0xa6, 0xe3, 0xa1),
        rose: Color::Rgb(0xf3, 0x8b, 0xa8),
        blue: Color::Rgb(0x89, 0xb4, 0xfa),
        teal: Color::Rgb(0x94, 0xe2, 0xd5),
        yellow: Color::Rgb(0xf9, 0xe2, 0xaf),
        peach: Color::Rgb(0xfa, 0xb3, 0x87),
        mauve: Color::Rgb(0xcb, 0xa6, 0xf7),
        green: Color::Rgb(0xa6, 0xda, 0x95),
        sky: Color::Rgb(0x74, 0xc7, 0xec),
    };

    const MACCHIATO: ThemeColors = ThemeColors {
        accent: Color::Rgb(0xb7, 0xbd, 0xf8), // lavender
        pill: Color::Rgb(0x8a, 0xad, 0xf4),   // mauveish? let's use blueish
        pill_fg: Color::Rgb(0xf4, 0xdb, 0xd6),
        text: Color::Rgb(0xca, 0xd3, 0xf5),
        dim: Color::Rgb(0x80, 0x87, 0xa2),
        faint: Color::Rgb(0x5b, 0x60, 0x78),
        border: Color::Rgb(0x49, 0x4d, 0x64),
        surface: Color::Rgb(0x36, 0x3a, 0x4f),
        sel_bg: Color::Rgb(0x24, 0x27, 0x3a),
        mint: Color::Rgb(0xa6, 0xda, 0x95),
        rose: Color::Rgb(0xf5, 0xa9, 0x7f),
        blue: Color::Rgb(0x8a, 0xad, 0xf4),
        teal: Color::Rgb(0x8b, 0xd5, 0xca),
        yellow: Color::Rgb(0xee, 0xd4, 0x9f),
        peach: Color::Rgb(0xf5, 0xa9, 0x7f),
        mauve: Color::Rgb(0xc6, 0xa0, 0xf6),
        green: Color::Rgb(0xa6, 0xda, 0x95),
        sky: Color::Rgb(0x91, 0xd7, 0xe3),
    };

    const LATTE: ThemeColors = ThemeColors {
        accent: Color::Rgb(0x72, 0x87, 0xfd), // lavender
        pill: Color::Rgb(0x88, 0x39, 0xef),   // mauve
        pill_fg: Color::Rgb(0xef, 0xf1, 0xf5),
        text: Color::Rgb(0x4c, 0x4f, 0x69),
        dim: Color::Rgb(0x6c, 0x6f, 0x85), // subtext0 for better contrast
        faint: Color::Rgb(0x8c, 0x8f, 0xa1), // overlay1
        border: Color::Rgb(0x9c, 0xa0, 0xb0), // overlay0
        surface: Color::Rgb(0xe6, 0xe9, 0xef),
        sel_bg: Color::Rgb(0xcc, 0xd0, 0xda),
        mint: Color::Rgb(0x40, 0xa0, 0x2b),
        rose: Color::Rgb(0xd2, 0x0f, 0x39),
        blue: Color::Rgb(0x1e, 0x66, 0xf5),
        teal: Color::Rgb(0x17, 0x92, 0x99),
        yellow: Color::Rgb(0xdf, 0x8e, 0x1d),
        peach: Color::Rgb(0xfe, 0x64, 0x0b),
        mauve: Color::Rgb(0x88, 0x39, 0xef),
        green: Color::Rgb(0x40, 0xa0, 0x2b),
        sky: Color::Rgb(0x04, 0xa5, 0xe5),
    };

    const TERMINAL: ThemeColors = ThemeColors {
        accent: Color::Magenta,
        pill: Color::Blue,
        pill_fg: Color::Reset,
        text: Color::Reset,
        dim: Color::DarkGray,
        faint: Color::DarkGray,
        border: Color::DarkGray,
        surface: Color::Reset,
        sel_bg: Color::DarkGray,
        mint: Color::Green,
        rose: Color::Red,
        blue: Color::Blue,
        teal: Color::Cyan,
        yellow: Color::Yellow,
        peach: Color::LightYellow,
        mauve: Color::Magenta,
        green: Color::Green,
        sky: Color::LightCyan,
    };

        const DRACULA: ThemeColors = ThemeColors {
        accent: Color::Rgb(0xff, 0x79, 0xc6), // pink
        pill: Color::Rgb(0xbd, 0x93, 0xf9),
        pill_fg: Color::Rgb(0xf8, 0xf8, 0xf2),
        text: Color::Rgb(0xf8, 0xf8, 0xf2),
        dim: Color::Rgb(0xbf, 0xbf, 0xbf),
        faint: Color::Rgb(0x62, 0x72, 0xa4),
        border: Color::Rgb(0x44, 0x47, 0x5a),
        surface: Color::Rgb(0x28, 0x2a, 0x36),
        sel_bg: Color::Rgb(0x44, 0x47, 0x5a),
        mint: Color::Rgb(0x50, 0xfa, 0x7b),
        rose: Color::Rgb(0xff, 0x55, 0x55),
        blue: Color::Rgb(0x8b, 0xe9, 0xfd),
        teal: Color::Rgb(0x8b, 0xe9, 0xfd),
        yellow: Color::Rgb(0xf1, 0xfa, 0x8c),
        peach: Color::Rgb(0xff, 0xb8, 0x6c),
        mauve: Color::Rgb(0xff, 0x79, 0xc6),
        green: Color::Rgb(0x50, 0xfa, 0x7b),
        sky: Color::Rgb(0x8b, 0xe9, 0xfd),
    };

    const NORD: ThemeColors = ThemeColors {
        accent: Color::Rgb(0x88, 0xc0, 0xd0),
        pill: Color::Rgb(0x81, 0xa1, 0xc1),
        pill_fg: Color::Rgb(0xec, 0xef, 0xf4),
        text: Color::Rgb(0xe5, 0xe9, 0xf0),
        dim: Color::Rgb(0xd8, 0xde, 0xe9),
        faint: Color::Rgb(0x4c, 0x56, 0x6a),
        border: Color::Rgb(0x3b, 0x42, 0x52),
        surface: Color::Rgb(0x2e, 0x34, 0x40),
        sel_bg: Color::Rgb(0x43, 0x4c, 0x5e),
        mint: Color::Rgb(0xa3, 0xbe, 0x8c),
        rose: Color::Rgb(0xbf, 0x61, 0x6a),
        blue: Color::Rgb(0x81, 0xa1, 0xc1),
        teal: Color::Rgb(0x8f, 0xbc, 0xbb),
        yellow: Color::Rgb(0xeb, 0xcb, 0x8b),
        peach: Color::Rgb(0xd0, 0x87, 0x70),
        mauve: Color::Rgb(0xb4, 0x8e, 0xad),
        green: Color::Rgb(0xa3, 0xbe, 0x8c),
        sky: Color::Rgb(0x88, 0xc0, 0xd0),
    };

    const TOKYO_NIGHT: ThemeColors = ThemeColors {
        accent: Color::Rgb(0xbb, 0x9a, 0xf7),
        pill: Color::Rgb(0x7a, 0xa2, 0xf7),
        pill_fg: Color::Rgb(0xc0, 0xca, 0xf5),
        text: Color::Rgb(0xc0, 0xca, 0xf5),
        dim: Color::Rgb(0xa9, 0xb1, 0xd6),
        faint: Color::Rgb(0x56, 0x5f, 0x89),
        border: Color::Rgb(0x29, 0x2e, 0x42),
        surface: Color::Rgb(0x1a, 0x1b, 0x26),
        sel_bg: Color::Rgb(0x29, 0x2e, 0x42),
        mint: Color::Rgb(0x9e, 0xce, 0x6a),
        rose: Color::Rgb(0xf7, 0x76, 0x8e),
        blue: Color::Rgb(0x7a, 0xa2, 0xf7),
        teal: Color::Rgb(0x7d, 0xcf, 0xff),
        yellow: Color::Rgb(0xe0, 0xaf, 0x68),
        peach: Color::Rgb(0xff, 0x9e, 0x64),
        mauve: Color::Rgb(0xbb, 0x9a, 0xf7),
        green: Color::Rgb(0x9e, 0xce, 0x6a),
        sky: Color::Rgb(0x7d, 0xcf, 0xff),
    };

    const SOLARIZED_LIGHT: ThemeColors = ThemeColors {
        accent: Color::Rgb(0x6c, 0x71, 0xc4),
        pill: Color::Rgb(0x26, 0x8b, 0xd2),
        pill_fg: Color::Rgb(0xfd, 0xf6, 0xe3),
        text: Color::Rgb(0x65, 0x7b, 0x83),
        dim: Color::Rgb(0x83, 0x94, 0x96),
        faint: Color::Rgb(0x93, 0xa1, 0xa1),
        border: Color::Rgb(0x93, 0xa1, 0xa1),
        surface: Color::Rgb(0xfd, 0xf6, 0xe3),
        sel_bg: Color::Rgb(0xee, 0xe8, 0xd5),
        mint: Color::Rgb(0x85, 0x99, 0x00),
        rose: Color::Rgb(0xdc, 0x32, 0x2f),
        blue: Color::Rgb(0x26, 0x8b, 0xd2),
        teal: Color::Rgb(0x2a, 0xa1, 0x98),
        yellow: Color::Rgb(0xb5, 0x89, 0x00),
        peach: Color::Rgb(0xcb, 0x4b, 0x16),
        mauve: Color::Rgb(0x6c, 0x71, 0xc4),
        green: Color::Rgb(0x85, 0x99, 0x00),
        sky: Color::Rgb(0x2a, 0xa1, 0x98),
    };

    const GRUVBOX_LIGHT: ThemeColors = ThemeColors {
        accent: Color::Rgb(0xb1, 0x62, 0x86),
        pill: Color::Rgb(0x45, 0x85, 0x88),
        pill_fg: Color::Rgb(0xfb, 0xf1, 0xc7),
        text: Color::Rgb(0x3c, 0x38, 0x36),
        dim: Color::Rgb(0x7c, 0x6f, 0x64),
        faint: Color::Rgb(0x92, 0x83, 0x74),
        border: Color::Rgb(0xa8, 0x99, 0x84),
        surface: Color::Rgb(0xfb, 0xf1, 0xc7),
        sel_bg: Color::Rgb(0xeb, 0xdb, 0xb2),
        mint: Color::Rgb(0x98, 0x97, 0x1a),
        rose: Color::Rgb(0xcc, 0x24, 0x1d),
        blue: Color::Rgb(0x45, 0x85, 0x88),
        teal: Color::Rgb(0x68, 0x9d, 0x6a),
        yellow: Color::Rgb(0xd7, 0x99, 0x21),
        peach: Color::Rgb(0xd6, 0x5d, 0x0e),
        mauve: Color::Rgb(0xb1, 0x62, 0x86),
        green: Color::Rgb(0x98, 0x97, 0x1a),
        sky: Color::Rgb(0x83, 0xa5, 0x98),
    };

    pub const THEME_NAMES: [&str; 9] = [
        "Terminal (Auto)",
        "Mocha (Dark)",
        "Macchiato (Dark)",
        "Latte (Light)",
        "Dracula",
        "Nord",
        "Tokyo Night",
        "Solarized (Light)",
        "Gruvbox (Light)"
    ];

    fn current() -> &'static ThemeColors {
        match crate::COLOR_THEME.load(std::sync::atomic::Ordering::Relaxed) {
            1 => &MOCHA,
            2 => &MACCHIATO,
            3 => &LATTE,
            4 => &DRACULA,
            5 => &NORD,
            6 => &TOKYO_NIGHT,
            7 => &SOLARIZED_LIGHT,
            8 => &GRUVBOX_LIGHT,
            _ => &TERMINAL,
        }
    }

    pub fn accent() -> Color { current().accent }
    pub fn pill() -> Color { current().pill }
    pub fn pill_fg() -> Color { current().pill_fg }
    pub fn text() -> Color { current().text }
    pub fn dim() -> Color { current().dim }
    pub fn faint() -> Color { current().faint }
    pub fn border() -> Color { current().border }
    pub fn surface() -> Color { current().surface }
    pub fn sel_bg() -> Color { current().sel_bg }
    pub fn mint() -> Color { current().mint }
    pub fn rose() -> Color { current().rose }
    pub fn blue() -> Color { current().blue }
    pub fn teal() -> Color { current().teal }
    pub fn yellow() -> Color { current().yellow }
    pub fn peach() -> Color { current().peach }
    pub fn mauve() -> Color { current().mauve }
    pub fn green() -> Color { current().green }
    pub fn sky() -> Color { current().sky }
}

/// One hue per device class, so color carries meaning across the whole UI.
pub fn class_color(class: u8) -> Color {
    match class {
        0x01 => theme::teal(),                // audio
        0x02 | 0x0a | 0xe0 => theme::green(), // comm / wireless
        0x03 => theme::blue(),                // HID
        0x06 | 0x0e | 0x10 => theme::mauve(), // imaging / video
        0x07 => theme::peach(),               // printer
        0x08 => theme::yellow(),              // storage
        0x09 => theme::sky(),                 // hub
        _ => theme::text(),
    }
}

/// One hue per PCI base class (numbering differs from USB: 0x01 = storage,
/// 0x02 = network, 0x03 = display …).
pub fn pci_class_color(class: u8) -> Color {
    match class {
        0x01 => theme::yellow(),        // mass storage
        0x02 => theme::green(),         // network
        0x03 => theme::mauve(),         // display
        0x04 => theme::teal(),          // multimedia
        0x06 => theme::sky(),           // bridge
        0x07 => theme::peach(),         // comm
        0x0c => theme::blue(),          // serial bus (USB/thunderbolt)
        0x0d => theme::green(),         // wireless
        _ => theme::text(),
    }
}

pub fn pci_icon(class: u8) -> &'static str {
    let theme = crate::ICON_THEME.load(std::sync::atomic::Ordering::Relaxed);
    if theme == 1 {
        match class {
            0x01 => "󰋊",
            0x02 => "󰲍",
            0x03 => "󰒋",
            0x04 => "󰎁",
            0x06 => "󰍩",
            0x07 => "󰗏",
            0x0c => "󰚥",
            0x0d => "󰤨",
            _ => "󰟥",
        }
    } else if theme == 2 {
        match class {
            0x01 => "[S]",
            0x02 => "[N]",
            0x03 => "[D]",
            0x04 => "[M]",
            0x06 => "[B]",
            0x07 => "[C]",
            0x0c => "[U]",
            0x0d => "[W]",
            _ => "[?]",
        }
    } else {
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
}

/// (tier glyph, human-readable speed, color) — brighter with tier.
pub fn speed_badge(speed: &str) -> Option<(&'static str, String, Color)> {
    let mbps: f32 = speed.parse().ok()?;
    let (glyph, color) = if mbps >= 5000.0 {
        ("█", theme::accent())
    } else if mbps >= 480.0 {
        ("▄", theme::sky())
    } else {
        ("▂", theme::dim())
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
        .border_style(Style::new().fg(theme::border()))
        .title(Line::from(format!(" {title} ").fg(theme::accent()).bold()))
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
        block.border_style(Style::new().fg(theme::accent()))
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
                    " ⏳ ".fg(theme::accent()).bold(),
                    format!("ejecting {label}…").fg(theme::text()),
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
                        .fg(if *ok { theme::mint() } else { theme::rose() })
                        .bold(),
                    msg.clone().fg(theme::text()),
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
            keys.extend([("p", "pci"), ("r", "rescan"), ("t", "theme"), ("q", "quit")]);
            let mut spans = vec![Span::raw(" ")];
            for (key, desc) in keys {
                spans.push(key.fg(theme::accent()).bold());
                spans.push(format!(" {desc}   ").fg(theme::dim()));
            }
            f.render_widget(Paragraph::new(Line::from(spans)), help);
        }

        // version pinned bottom-right; upgrade badge when a newer release exists
        let ver = env!("CARGO_PKG_VERSION");
        let right = match &app.update {
            Some(new) => Line::from(vec![
                format!("v{ver} ").fg(theme::dim()),
                format!("↑ v{new} ").fg(theme::mint()).bold(),
            ]),
            None => Line::from(format!("v{ver} ").fg(theme::faint())),
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
                        item.style(Style::new().bg(theme::sel_bg()).fg(theme::accent()))
                    } else {
                        item
                    }
                })
                .collect();
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(theme::accent()))
                .title(Line::from(" copy ".fg(theme::accent()).bold()));
            f.render_widget(
                List::new(items).style(Style::new().fg(theme::text())).block(block),
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
                Line::from(format!("Safely eject {name}?").fg(theme::text()).bold()),
                Line::from(label.fg(theme::dim())),
                Line::from(""),
                Line::from(vec![
                    " unmounts and powers off the drive".fg(theme::faint()),
                ]),
                Line::from(""),
                Line::from(vec![
                    " e ".fg(theme::pill_fg()).bg(theme::pill()).bold(),
                    " confirm    ".fg(theme::dim()),
                    " esc ".fg(theme::text()).bg(theme::surface()),
                    " cancel".fg(theme::dim()),
                ]),
            ];
            let w = body.iter().map(Line::width).max().unwrap_or(30) as u16 + 4;
            let rect = centered(app.screen, w, body.len() as u16 + 2);
            f.render_widget(Clear, rect);
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(theme::rose()))
                .title(Line::from(" eject ".fg(theme::rose()).bold()))
                .padding(Padding::horizontal(1));
            f.render_widget(Paragraph::new(body).block(block), rect);
        }

        if let Some((idx, _)) = app.theme_picker {
            let mut items: Vec<ListItem> = Vec::new();
            for (i, name) in theme::THEME_NAMES.iter().enumerate() {
                let text = if i == idx {
                    format!(" > {} ", name)
                } else {
                    format!("   {} ", name)
                };
                let style = if i == idx {
                    Style::new().bg(theme::sel_bg()).fg(theme::accent()).bold()
                } else {
                    Style::new().fg(theme::text())
                };
                items.push(ListItem::new(text).style(style));
            }
            
            let w = theme::THEME_NAMES.iter().map(|n| n.len()).max().unwrap_or(20) as u16 + 8;
            let rect = centered(app.screen, w, items.len() as u16 + 2);
            f.render_widget(Clear, rect);
            
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(theme::accent()))
                .title(Line::from(" theme ".fg(theme::accent()).bold()));
            
            f.render_widget(List::new(items).block(block), rect);
        }
    }

    pub fn draw_header(app: &App, f: &mut Frame, area: Rect) {
        let buses = app.devices.iter().filter(|d| d.is_root_hub()).count();
        let up = app.started.elapsed().as_secs();
        let line = Line::from(vec![
            Span::styled(
                " usbtree ",
                Style::new().bg(theme::pill()).fg(theme::pill_fg()).bold(),
            ),
            Span::raw("  "),
            app.devices.len().to_string().fg(theme::text()).bold(),
            " devices".fg(theme::dim()),
            "  ·  ".fg(theme::faint()),
            buses.to_string().fg(theme::text()).bold(),
            " buses".fg(theme::dim()),
            "  ·  ".fg(theme::faint()),
            format!("up {:02}:{:02}", up / 60, up % 60).fg(theme::dim()),
            "  ·  ".fg(theme::faint()),
            app.metrics.header_note().fg(if app.metrics.is_bytes() {
                theme::mint()
            } else {
                theme::dim()
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
                    " / ".fg(theme::accent()).bold(),
                    format!("{}{}", flt.query, if flt.editing { "▏" } else { "" }).fg(theme::text()),
                    format!("  {n} match{} ", if n == 1 { "" } else { "es" }).fg(theme::dim()),
                ]);
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(theme::border()))
                    .title(title)
                    .padding(Padding::horizontal(1))
            }
            None => pane("✦ tree"),
        };
        let block = focus_ring(block, app.focus == Focus::Tree);
        if app.filter.is_some() && app.rows.is_empty() {
            f.render_widget(
                Paragraph::new("no matches".fg(theme::dim()).italic()).block(block),
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
                    (Some(t), _) => lerp(theme::rose(), theme::dim(), t),
                    (_, Some(t)) => lerp(theme::mint(), base, t),
                    _ => base,
                };

                let mut spans = vec![
                    if selected == Some(row) {
                        "▌ ".fg(theme::accent())
                    } else {
                        Span::raw("  ")
                    },
                    // fixed-width class gutter: aligned column, easy to scan
                    Span::styled(
                        format!(" {:<8.8} ", d.class_name()),
                        Style::new()
                            .fg(fade(class_color(d.effective_class())))
                            .bg(theme::surface()),
                    ),
                    Span::raw(" "),
                    rails[row].clone().fg(theme::faint()),
                ];
                let kids = usb::child_count(&app.render, &d.name);
                let folded = app.collapsed.contains(&d.name);
                spans.push(if kids == 0 {
                    Span::raw("  ")
                } else if folded {
                    "▸ ".fg(theme::accent()).bold()
                } else {
                    "▾ ".fg(theme::faint())
                });

                spans.push(format!("{:<8}", d.name).fg(fade(theme::dim())));
                spans.push(Span::raw(format!(" {} ", d.icon())));
                let label_color = fade(if d.is_root_hub() {
                    theme::accent()
                } else {
                    theme::text()
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
                        Style::new().bg(theme::pill()).fg(theme::pill_fg()).bold(),
                    ));
                }

                // right-aligned block, fixed-width columns so they stack tidily:
                // [spark:SPARK_WIDTH] [rate:RATE_W] [badge — only when present]
                const RATE_W: usize = 9;
                let h = app.rates.get(&d.name);
                let has_traffic =
                    h.is_some_and(|h| h.iter().rev().take(SPARK_WIDTH).any(|&v| v > 0));
                let badge = match (ghost.is_some(), flash.is_some()) {
                    (true, _) => Some(("○ unplugged", fade(theme::text()))),
                    (_, true) => Some(("● plugged", fade(theme::text()))),
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
                    let metric = fade(theme::mint());
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
            .style(Style::new().fg(theme::text()))
            .block(block)
            .scroll_padding(2)
            .highlight_style(Style::new().bg(theme::sel_bg()));
        f.render_stateful_widget(list, area, &mut app.list);
    }

    pub fn draw_detail(app: &App, f: &mut Frame, area: Rect) {
        let block = pane("details");
        let Some(&(_, i)) = app.list.selected().and_then(|s| app.rows.get(s)) else {
            f.render_widget(block, area);
            return;
        };
        let d = &app.render[i];
        let key = |k: &str| format!("{k:<10}").fg(theme::dim());
        let mut lines = vec![
            Line::from(format!("{} {}", d.icon(), d.label()).fg(theme::text()).bold()),
            Line::from(d.vendor_name().fg(theme::dim())),
            Line::from("─".repeat(24).fg(theme::faint())),
            Line::from(vec![key("sysfs"), d.name.clone().fg(theme::text())]),
            Line::from(vec![
                key("vid:pid"),
                format!("{:04x}:{:04x}", d.vid, d.pid).fg(theme::accent()),
            ]),
            Line::from(vec![
                key("class"),
                d.class_name().fg(class_color(d.effective_class())),
                // device-level triple bDeviceClass:SubClass:Protocol (name is the
                // friendly effective class; the code is the raw device descriptor)
                format!("  {:02x}:{:02x}:{:02x}", d.class, d.subclass, d.protocol)
                    .fg(theme::faint()),
            ]),
        ];
        if d.usb_version != 0 {
            lines.push(Line::from(vec![
                key("usb"),
                bcd(d.usb_version).fg(theme::text()),
                "  spec".fg(theme::faint()),
            ]));
        }
        if d.device_version != 0 {
            lines.push(Line::from(vec![
                key("rev"),
                bcd(d.device_version).fg(theme::text()),
            ]));
        }
        if let Some((glyph, human, color)) = speed_badge(&d.speed) {
            lines.push(Line::from(vec![
                key("speed"),
                format!("{glyph} {human}").fg(color),
                format!("  {} Mbps", d.speed).fg(theme::faint()),
            ]));
        }
        if let Some(ma) = d.max_power_ma {
            lines.push(Line::from(vec![
                key("power"),
                format!("{ma} mA").fg(theme::text()),
                "  max".fg(theme::faint()),
            ]));
        }
        if let Some(a) = d.config_attributes {
            let mut spans = vec![
                key("powered"),
                if a & 0x40 != 0 { "app" } else { "bus" }.fg(theme::text()),
            ];
            if a & 0x20 != 0 {
                spans.push("  remote-wakeup".fg(theme::faint()));
            }
            lines.push(Line::from(spans));
        }
        if let Some(s) = &d.serial {
            lines.push(Line::from(vec![key("serial"), s.clone().fg(theme::text())]));
        }
        let kids = usb::child_count(&app.render, &d.name);
        if kids > 0 {
            lines.push(Line::from(vec![
                key("connected"),
                kids.to_string().fg(theme::text()),
            ]));
        }
        if !d.interfaces.is_empty() {
            lines.push(Line::from("─".repeat(24).fg(theme::faint())));
            lines.push(Line::from("interfaces".fg(theme::dim())));
            for i in &d.interfaces {
                let mut head = vec![
                    format!("{:>2} ", i.number).fg(theme::faint()),
                    usb::class_name(i.class).fg(class_color(i.class)),
                ];
                if let Some(n) = &i.name {
                    head.push(format!("  {n}").fg(theme::text()));
                }
                head.push(
                    format!("  {:02x}:{:02x}:{:02x}", i.class, i.subclass, i.protocol)
                        .fg(theme::faint()),
                );
                if i.alt != 0 {
                    head.push(format!("  alt{}", i.alt).fg(theme::faint()));
                }
                lines.push(Line::from(head));
                for e in &i.endpoints {
                    lines.push(Line::from(vec![
                        format!("    {:#04x} ", e.address).fg(theme::dim()),
                        if e.input { "IN  " } else { "OUT " }.fg(theme::accent()),
                        format!("{:<4} ", ep_type(e.transfer)).fg(theme::text()),
                        format!("{}B", e.max_packet).fg(theme::dim()),
                        match e.transfer {
                            1 | 3 if e.interval != 0 => format!("  @{}", e.interval),
                            _ => String::new(),
                        }
                        .fg(theme::faint()),
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
                    if bytes { "bandwidth " } else { "activity " }.fg(theme::dim()),
                    fmt_rate(cur, bytes).fg(theme::mint()).bold(),
                    if bytes { "" } else { " URBs" }.fg(theme::faint()),
                ])),
                title,
            );
            f.render_widget(
                Sparkline::default()
                    .data(h)
                    .style(Style::new().fg(theme::mint())),
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
                    "waiting for hot-plug events…".fg(theme::dim()).italic(),
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
            list = list.highlight_style(Style::new().bg(theme::sel_bg()));
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
            ("t", "theme"),
            ("q", "quit"),
        ];
        let mut spans = vec![Span::raw(" ")];
        for (key, desc) in keys {
            spans.push(key.fg(theme::accent()).bold());
            spans.push(format!(" {desc}   ").fg(theme::dim()));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), help);
        let ver = env!("CARGO_PKG_VERSION");
        f.render_widget(
            Paragraph::new(Line::from(format!("v{ver} ").fg(theme::faint())))
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
                Style::new().bg(theme::pill()).fg(theme::pill_fg()).bold(),
            ),
            "  ".into(),
            "usb".fg(theme::dim()),
            " · ".fg(theme::faint()),
            "pci".fg(theme::accent()).bold(),
            "  ·  ".fg(theme::faint()),
            app.pci.len().to_string().fg(theme::text()).bold(),
            " devices".fg(theme::dim()),
            "  ·  ".fg(theme::faint()),
            buses.to_string().fg(theme::text()).bold(),
            " buses".fg(theme::dim()),
        ]);
        f.render_widget(Paragraph::new(line), area);
    }

    pub fn draw_pci_tree(app: &mut App, f: &mut Frame, area: Rect) {
        let block = match &app.filter {
            Some(flt) => {
                let title = Line::from(vec![
                    " / ".fg(theme::accent()).bold(),
                    format!("{}{}", flt.query, if flt.editing { "▏" } else { "" }).fg(theme::text()),
                ]);
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(theme::accent()))
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
            f.render_widget(Paragraph::new(msg.fg(theme::dim()).italic()).block(block), area);
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
                    "▌ ".fg(theme::accent())
                } else {
                    Span::raw("  ")
                };
                match r {
                    PciRow::Bus(bus) => ListItem::new(Line::from(vec![
                        sel,
                        format!(" {bus} ").fg(theme::accent()).bold(),
                    ])),
                    PciRow::Dev(i) => {
                        let d = &app.pci[*i];
                        ListItem::new(Line::from(vec![
                            sel,
                            Span::styled(
                                format!(" {:<10.10} ", d.class_group()),
                                Style::new().fg(pci_class_color(d.class)).bg(theme::surface()),
                            ),
                            Span::raw(" "),
                            rails[row].clone().fg(theme::faint()),
                            format!("{} ", pci_icon(d.class)).into(),
                            format!("{} ", d.label()).fg(theme::text()),
                            d.addr.clone().fg(theme::faint()),
                        ]))
                    }
                }
            })
            .collect();
        let list = List::new(items)
            .style(Style::new().fg(theme::text()))
            .block(block)
            .scroll_padding(2)
            .highlight_style(Style::new().bg(theme::sel_bg()));
        f.render_stateful_widget(list, area, &mut app.pci_list);
    }

    pub fn draw_pci_detail(app: &App, f: &mut Frame, area: Rect) {
        let block = pane("details");
        let Some(i) = app.pci_selected() else {
            f.render_widget(block, area);
            return;
        };
        let d = &app.pci[i];
        let key = |k: &str| format!("{k:<10}").fg(theme::dim());
        let mut lines = vec![
            Line::from(format!("{} {}", pci_icon(d.class), d.label()).fg(theme::text()).bold()),
            Line::from(d.vendor_name().fg(theme::dim())),
            Line::from("─".repeat(24).fg(theme::faint())),
            Line::from(vec![key("address"), d.addr.clone().fg(theme::text())]),
            Line::from(vec![
                key("vid:pid"),
                format!("{:04x}:{:04x}", d.vid, d.pid).fg(theme::accent()),
            ]),
            Line::from(vec![
                key("class"),
                d.class_name().fg(pci_class_color(d.class)),
                format!("  {:02x}:{:02x}:{:02x}", d.class, d.subclass, d.prog_if).fg(theme::faint()),
            ]),
        ];
        if let Some(pif) = d.prog_if_name() {
            lines.push(Line::from(vec![key("interface"), pif.fg(theme::text())]));
        }
        lines.push(Line::from(vec![
            key("revision"),
            format!("{:02x}", d.revision).fg(theme::text()),
        ]));
        if let Some((v, p)) = d.subsystem {
            let val = d.subsystem_name().unwrap_or_else(|| format!("{v:04x}:{p:04x}"));
            lines.push(Line::from(vec![key("subsystem"), val.fg(theme::text())]));
        }
        if let Some(l) = &d.link {
            let mut spans = vec![
                key("link"),
                format!("x{} {}", l.cur_width, l.cur_speed).fg(theme::text()),
            ];
            if l.throttled() {
                spans.push(format!("  ↓max x{} {}", l.max_width, l.max_speed).fg(theme::peach()));
            }
            lines.push(Line::from(spans));
        }
        if let Some(drv) = &d.driver {
            lines.push(Line::from(vec![key("driver"), drv.clone().fg(theme::text())]));
        }
        if let Some(n) = d.numa_node {
            lines.push(Line::from(vec![key("numa"), n.to_string().fg(theme::text())]));
        }
        if let Some(g) = &d.iommu_group {
            lines.push(Line::from(vec![key("iommu"), g.clone().fg(theme::text())]));
        }
        if let Some(ps) = &d.power_state {
            lines.push(Line::from(vec![key("power"), ps.clone().fg(theme::text())]));
        }
        f.render_widget(Paragraph::new(lines).block(block), area);
    }
