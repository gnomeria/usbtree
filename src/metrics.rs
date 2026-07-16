//! Live per-device activity. Unprivileged: URB-count deltas from sysfs
//! `urbnum`. When usbmon is readable (root + debugfs): real bytes/s.

use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io::{BufRead, BufReader};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};

use crate::usb::Device;

#[cfg(target_os = "linux")]
const USBMON: &str = "/sys/kernel/debug/usb/usbmon/0u";

/// Why usbmon bytes/s isn't active, distinguished by the `open` error kind so
/// the header can tell the user the *actual* fix (module vs root).
#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
pub enum NoBytes {
    /// usbmon file missing (`ENOENT`): module not loaded or debugfs unmounted.
    NeedModule,
    /// present but unreadable (`EACCES`): not running as root.
    NeedRoot,
    /// present, running as root, but `EPERM` from kernel lockdown (Secure
    /// Boot): even root can't read usbmon until lockdown is turned off.
    Locked,
}

pub enum Metrics {
    /// URBs/s per device from `/sys/bus/usb/devices/*/urbnum`.
    #[cfg(target_os = "linux")]
    Urb {
        prev: HashMap<String, u64>,
        why: NoBytes,
    },
    /// Bytes/s per (bus, devnum) accumulated by a usbmon reader thread.
    #[cfg(target_os = "linux")]
    UsbMon { bytes: Arc<Mutex<HashMap<(u16, u16), u64>>> },
    /// Synthetic bytes/s for `--demo`, deterministic per device and tick.
    Demo { tick: u64 },
    /// No per-device activity source (macOS/Windows: nothing published
    /// unprivileged — the sole IOKit counter is HID-only, not worth it).
    #[cfg(not(target_os = "linux"))]
    None,
}

impl Metrics {
    #[cfg(target_os = "linux")]
    pub fn new() -> Self {
        match fs::File::open(USBMON) {
            Ok(f) => {
                let bytes: Arc<Mutex<HashMap<(u16, u16), u64>>> = Arc::default();
                let sink = Arc::clone(&bytes);
                std::thread::spawn(move || {
                    for line in BufReader::new(f).lines() {
                        let Ok(line) = line else { break };
                        if let Some((key, len)) = parse_usbmon(&line) {
                            *sink.lock().unwrap().entry(key).or_insert(0) += len;
                        }
                    }
                });
                Metrics::UsbMon { bytes }
            }
            Err(e) => {
                let why = if e.kind() == std::io::ErrorKind::NotFound {
                    NoBytes::NeedModule
                } else if e.raw_os_error() == Some(1) && lockdown_active() {
                    // EPERM (1) despite root = kernel lockdown, not DAC perms.
                    // Confirm against the LSM mode file so a stray EPERM from
                    // some other cause doesn't get mislabeled as lockdown.
                    NoBytes::Locked
                } else {
                    NoBytes::NeedRoot
                };
                Metrics::Urb {
                    prev: HashMap::new(),
                    why,
                }
            }
        }
    }

    // ponytail: macOS/Windows have no unprivileged per-device traffic counter
    // (urbnum is Linux sysfs; IOKit only exposes HID InputReportCount). Show
    // the tree, no rates. Upgrade path: HID-only blink via ioreg if wanted.
    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Self {
        Metrics::None
    }

    pub fn demo() -> Self {
        Metrics::Demo { tick: 0 }
    }

    /// True when rates are real bytes (usbmon), not URB counts.
    pub fn is_bytes(&self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            Metrics::UsbMon { .. } => true,
            Metrics::Demo { .. } => true,
            _ => false,
        }
    }

    /// Header indicator text: the active source, or — when on URB fallback —
    /// the specific fix to unlock bytes/s (`modprobe usbmon` vs `sudo`).
    pub fn header_note(&self) -> &'static str {
        match self {
            #[cfg(target_os = "linux")]
            Metrics::UsbMon { .. } => "◉ usbmon bytes/s",
            Metrics::Demo { .. } => "◉ usbmon bytes/s",
            #[cfg(target_os = "linux")]
            Metrics::Urb { why, .. } => match why {
                NoBytes::NeedModule => "◌ urb activity — modprobe usbmon for bytes/s",
                NoBytes::NeedRoot => "◌ urb activity — sudo + modprobe usbmon for bytes/s",
                NoBytes::Locked => "◌ urb activity — kernel lockdown blocks usbmon (disable Secure Boot)",
            },
            #[cfg(not(target_os = "linux"))]
            Metrics::None => "◌ activity n/a on this platform",
        }
    }

    /// Per-device rate accumulated since the last call, keyed by sysfs name.
    pub fn sample(&mut self, devices: &[Device]) -> HashMap<String, u64> {
        match self {
            #[cfg(target_os = "linux")]
            Metrics::Urb { prev, .. } => {
                let mut out = HashMap::new();
                let mut cur = HashMap::new();
                for d in devices {
                    let path = format!("/sys/bus/usb/devices/{}/urbnum", d.name);
                    let Some(n) = read_u64(&path) else { continue };
                    let base = prev.get(&d.name).copied().unwrap_or(n);
                    out.insert(d.name.clone(), n.saturating_sub(base));
                    cur.insert(d.name.clone(), n);
                }
                *prev = cur;
                out
            }
            #[cfg(target_os = "linux")]
            Metrics::UsbMon { bytes } => {
                let drained = std::mem::take(&mut *bytes.lock().unwrap());
                devices
                    .iter()
                    .filter_map(|d| {
                        let key = (d.busnum()?, d.devnum as u16);
                        Some((d.name.clone(), *drained.get(&key)?))
                    })
                    .collect()
            }
            Metrics::Demo { tick } => {
                *tick += 1;
                let t = *tick;
                devices
                    .iter()
                    .map(|d| (d.name.clone(), demo_rate(d, t)))
                    .filter(|&(_, r)| r > 0)
                    .collect()
            }
            #[cfg(not(target_os = "linux"))]
            Metrics::None => HashMap::new(),
        }
    }
}

/// Plausible traffic per class, real-world scale: a 2-in/2-out audio interface
/// trickles ~0.5 MB/s, a 1080p webcam ~3 MB/s (H.264), a USB SSD bursts to a
/// few hundred MB/s during a copy then idles, HID barely registers.
fn demo_rate(d: &Device, t: u64) -> u64 {
    let phase: u64 = d.name.bytes().map(u64::from).sum();
    let wave = (((t + phase) as f64) * 0.9).sin() * 0.5 + 0.5; // 0..1
    let base = match d.effective_class() {
        0x01 => 700_000.0,
        0x0e => 3_200_000.0,
        0x08 => {
            if (t + phase) % 11 < 5 {
                160_000_000.0
            } else {
                400_000.0
            }
        }
        0x03 if (t + phase).is_multiple_of(3) => 1_800.0,
        _ => 0.0,
    };
    (base * (0.6 + 0.4 * wave)) as u64
}

#[cfg(target_os = "linux")]
fn read_u64(path: &str) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Kernel lockdown active? Reads the LSM's mode file. `[none]` bracketed (or
/// the file absent = LSM not built) means off; any other bracketed mode
/// (`integrity`/`confidentiality`) blocks usbmon reads even for root.
#[cfg(target_os = "linux")]
fn lockdown_active() -> bool {
    fs::read_to_string("/sys/kernel/security/lockdown").is_ok_and(|s| lockdown_on(&s))
}

/// The bracket check, split out so it's testable without the file. The active
/// mode is the bracketed word: `[none] integrity confidentiality` = off.
#[cfg(any(target_os = "linux", test))]
fn lockdown_on(mode: &str) -> bool {
    !mode.contains("[none]")
}

/// Parse one usbmon text line, e.g.
/// `ffff9c.. 3003687252 C Ii:1:002:1 0:8 8 = 1f00..` -> ((bus, dev), bytes).
/// Counts completed IN and submitted OUT transfers (usbtop's method).
// ponytail: control transfers with inline setup ('s' status word) are
// skipped — a few bytes each, not worth the extra field shuffling
#[cfg(any(target_os = "linux", test))]
fn parse_usbmon(line: &str) -> Option<((u16, u16), u64)> {
    let mut f = line.split_whitespace();
    let (_tag, _ts) = (f.next()?, f.next()?);
    let event = f.next()?;
    let addr = f.next()?;
    if f.next()? == "s" {
        return None;
    }
    let len: u64 = f.next()?.parse().ok()?;
    let mut a = addr.split(':');
    let dir = a.next()?.chars().nth(1)?; // "Ii" -> 'i', "Bo" -> 'o'
    let bus: u16 = a.next()?.parse().ok()?;
    let dev: u16 = a.next()?.parse().ok()?;
    match (event, dir) {
        ("C", 'i') | ("S", 'o') => Some(((bus, dev), len)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
