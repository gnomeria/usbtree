//! Live per-device activity. Unprivileged: URB-count deltas from sysfs
//! `urbnum`. When usbmon is readable (root + debugfs): real bytes/s.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};

use crate::usb::Device;

const USBMON: &str = "/sys/kernel/debug/usb/usbmon/0u";

pub enum Metrics {
    /// URBs/s per device from `/sys/bus/usb/devices/*/urbnum`.
    Urb { prev: HashMap<String, u64> },
    /// Bytes/s per (bus, devnum) accumulated by a usbmon reader thread.
    UsbMon { bytes: Arc<Mutex<HashMap<(u16, u16), u64>>> },
    /// Synthetic bytes/s for `--demo`, deterministic per device and tick.
    Demo { tick: u64 },
}

impl Metrics {
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
            Err(_) => Metrics::Urb { prev: HashMap::new() },
        }
    }

    pub fn demo() -> Self {
        Metrics::Demo { tick: 0 }
    }

    /// True when rates are real bytes (usbmon), not URB counts.
    pub fn is_bytes(&self) -> bool {
        matches!(self, Metrics::UsbMon { .. } | Metrics::Demo { .. })
    }

    /// Per-device rate accumulated since the last call, keyed by sysfs name.
    pub fn sample(&mut self, devices: &[Device]) -> HashMap<String, u64> {
        match self {
            Metrics::Urb { prev } => {
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
        }
    }
}

/// Plausible traffic per class: steady audio, bursty storage, trickling HID.
fn demo_rate(d: &Device, t: u64) -> u64 {
    let phase: u64 = d.name.bytes().map(u64::from).sum();
    let wave = (((t + phase) as f64) * 0.9).sin() * 0.5 + 0.5; // 0..1
    let base = match d.effective_class() {
        0x01 => 12_000_000.0,
        0x0e => 24_000_000.0,
        0x08 => {
            if (t + phase) % 11 < 5 {
                280_000_000.0
            } else {
                400_000.0
            }
        }
        0x03 if (t + phase).is_multiple_of(3) => 1_800.0,
        _ => 0.0,
    };
    (base * (0.6 + 0.4 * wave)) as u64
}

fn read_u64(path: &str) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Parse one usbmon text line, e.g.
/// `ffff9c.. 3003687252 C Ii:1:002:1 0:8 8 = 1f00..` -> ((bus, dev), bytes).
/// Counts completed IN and submitted OUT transfers (usbtop's method).
// ponytail: control transfers with inline setup ('s' status word) are
// skipped — a few bytes each, not worth the extra field shuffling
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
mod tests {
    use super::*;

    #[test]
    fn usbmon_parse() {
        // completed interrupt IN: counted
        let l = "ffff9c1a 3003687252 C Ii:1:002:1 0:8 8 = 1f000000";
        assert_eq!(parse_usbmon(l), Some(((1, 2), 8)));
        // submitted bulk OUT: counted
        let l = "ffff9c1a 3003687252 S Bo:2:005:2 -115 512 = aa";
        assert_eq!(parse_usbmon(l), Some(((2, 5), 512)));
        // submitted IN (no data moved yet): not counted
        let l = "ffff9c1a 3003687252 S Ii:1:002:1 -115:8 8 <";
        assert_eq!(parse_usbmon(l), None);
        // control setup: skipped
        let l = "ffff9c1a 3003687252 S Co:1:001:0 s 23 01 0010 0002 0000 0";
        assert_eq!(parse_usbmon(l), None);
        assert_eq!(parse_usbmon("garbage"), None);
    }
}
