use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use nusb::MaybeFuture;
use usb_ids::FromId;

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    /// tree path, sysfs-style: "usb1" (root hub) or "1-1.4" (bus 1, port 1, port 4)
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub speed: String,
    pub class: u8,
    /// classes of all interfaces (composite devices describe themselves here)
    pub iface_classes: Vec<u8>,
    /// bus device number, for matching usbmon traffic
    pub devnum: u8,
}

impl Device {
    /// Best human-readable name: user overrides, sysfs product string,
    /// usb.ids, then vendor + class heuristics.
    pub fn label(&self) -> String {
        if let Some(name) = overrides().get(&(self.vid, self.pid)) {
            return name.clone();
        }
        if let Some(p) = &self.product {
            return p.clone();
        }
        if let Some(n) = ids_product(self.vid, self.pid) {
            return n;
        }
        match self.vendor_lookup() {
            Some(v) => format!("{} {}", v, self.class_name()),
            None => format!("Unknown device {:04x}:{:04x}", self.vid, self.pid),
        }
    }

    fn vendor_lookup(&self) -> Option<String> {
        self.manufacturer
            .clone()
            .or_else(|| ids_vendor(self.vid))
    }

    pub fn vendor_name(&self) -> String {
        self.vendor_lookup()
            .unwrap_or_else(|| format!("{:04x}", self.vid))
    }

    pub fn class_name(&self) -> &'static str {
        class_name(self.effective_class())
    }

    pub fn icon(&self) -> &'static str {
        if self.is_root_hub() {
            return "🖥️";
        }
        match self.effective_class() {
            0x01 => "🔊",
            0x02 | 0x0a => "📡",
            0x03 => "⌨️",
            0x06 => "📷",
            0x07 => "🖨️",
            0x08 => "💾",
            0x09 => "🔌",
            0x0b => "💳",
            0x0d => "🔒",
            0x0e => "🎥",
            0x10 => "🎬",
            0xe0 => "📶",
            _ => "🔹",
        }
    }

    /// Device class; composite (0x00) and IAD/Misc (0xef) devices are
    /// classified by their most meaningful interface class instead —
    /// e.g. a MOTU M2 reports 0xef at device level but Audio interfaces.
    pub fn effective_class(&self) -> u8 {
        if self.class != 0 && self.class != 0xef {
            return self.class;
        }
        const PRIORITY: [u8; 11] = [
            0x0e, 0x01, 0x10, 0x08, 0x07, 0x06, 0x03, 0xe0, 0x02, 0x0b, 0x0d,
        ];
        for c in PRIORITY {
            if self.iface_classes.contains(&c) {
                return c;
            }
        }
        *self.iface_classes.first().unwrap_or(&self.class)
    }

    pub fn is_root_hub(&self) -> bool {
        self.name.starts_with("usb")
    }

    /// Bus number parsed from the sysfs name: "usb1" or "1-1.4" -> 1.
    pub fn busnum(&self) -> Option<u16> {
        match self.name.strip_prefix("usb") {
            Some(b) => b.parse().ok(),
            None => self.name.split('-').next()?.parse().ok(),
        }
    }

    /// Identity key for hot-plug diffing.
    pub fn key(&self) -> String {
        format!("{} {:04x}:{:04x}", self.name, self.vid, self.pid)
    }

    /// sysfs name of the parent device: "1-1.4.2" -> "1-1.4", "1-1" -> "usb1".
    pub fn parent_name(&self) -> Option<String> {
        if self.is_root_hub() {
            return None;
        }
        let (bus, path) = self.name.split_once('-')?;
        match path.rsplit_once('.') {
            Some((rest, _)) => Some(format!("{bus}-{rest}")),
            None => Some(format!("usb{bus}")),
        }
    }
}

pub fn class_name(class: u8) -> &'static str {
    // ponytail: local table, usb.ids class section not exposed as simply by the crate
    match class {
        0x01 => "Audio",
        0x02 => "Comm",
        0x03 => "HID",
        0x05 => "Physical",
        0x06 => "Imaging",
        0x07 => "Printer",
        0x08 => "Storage",
        0x09 => "Hub",
        0x0a => "CDC-Data",
        0x0b => "Smart Card",
        0x0d => "Security",
        0x0e => "Video",
        0x0f => "Health",
        0x10 => "AV",
        0xdc => "Diagnostic",
        0xe0 => "Wireless",
        0xef => "Misc",
        0xfe => "App-specific",
        0xff => "Vendor-specific",
        _ => "Device",
    }
}

/// Enumerate USB devices via nusb (Linux, macOS, Windows).
/// Root hubs are synthesized from the bus list; device tree paths are built
/// from each device's port chain, matching Linux sysfs naming.
pub fn scan() -> Vec<Device> {
    let mut devices = Vec::new();
    if let Ok(buses) = nusb::list_buses().wait() {
        for bus in buses {
            devices.push(Device {
                name: format!("usb{}", tidy_bus(bus.bus_id())),
                vid: 0,
                pid: 0,
                manufacturer: bus.driver().map(str::to_string),
                product: Some(
                    bus.system_name()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("Bus {}", bus.bus_id())),
                ),
                serial: None,
                speed: String::new(),
                class: 0x09,
                iface_classes: Vec::new(),
                devnum: 1, // root hub is always device 1 on its bus
            });
        }
    }
    let Ok(list) = nusb::list_devices().wait() else {
        return devices;
    };
    for d in list {
        let chain = d.port_chain();
        if chain.is_empty() {
            continue; // root hub; already synthesized from the bus list
        }
        let path: Vec<String> = chain.iter().map(u8::to_string).collect();
        devices.push(Device {
            name: format!("{}-{}", tidy_bus(d.bus_id()), path.join(".")),
            vid: d.vendor_id(),
            pid: d.product_id(),
            manufacturer: d.manufacturer_string().map(str::to_string),
            product: d.product_string().map(str::to_string),
            serial: d.serial_number().map(str::to_string),
            speed: d.speed().map(speed_mbps).unwrap_or_default(),
            class: d.class(),
            iface_classes: d
                .interfaces()
                .map(|i| i.class())
                .filter(|&c| c != 0)
                .collect(),
            devnum: d.device_address(),
        });
    }
    devices.sort_by_key(|d| sort_key(&d.name));
    devices
}

/// "001" (zero-padded on Linux) -> "1", keeps non-numeric ids as-is.
fn tidy_bus(id: &str) -> &str {
    let t = id.trim_start_matches('0');
    if t.is_empty() && !id.is_empty() {
        "0"
    } else if t.len() != id.len() && !t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        id // "0abc" style id: zeros were significant
    } else {
        t
    }
}

fn speed_mbps(s: nusb::Speed) -> String {
    match s {
        nusb::Speed::Low => "1.5".into(),
        nusb::Speed::Full => "12".into(),
        nusb::Speed::High => "480".into(),
        nusb::Speed::Super => "5000".into(),
        nusb::Speed::SuperPlus => "10000".into(),
        _ => String::new(),
    }
}

/// User heuristics DB: `~/.config/usbtree/overrides.ids`, one entry per line:
/// `vvvv:pppp Friendly Name`. Wins over sysfs strings and usb.ids.
fn overrides() -> &'static HashMap<(u16, u16), String> {
    static OVERRIDES: OnceLock<HashMap<(u16, u16), String>> = OnceLock::new();
    OVERRIDES.get_or_init(|| {
        overrides_path()
            .and_then(|p| fs::read_to_string(p).ok())
            .map(|s| parse_overrides(&s))
            .unwrap_or_default()
    })
}

fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(base.join("usbtree"))
}

fn overrides_path() -> Option<PathBuf> {
    Some(config_dir()?.join("overrides.ids"))
}

// ---- downloadable usb.ids (usbtree --updatelist) ----

const USB_IDS_URL: &str = "https://raw.githubusercontent.com/systemd/hwdata/main/usb.ids";

pub fn ids_cache_path() -> Option<PathBuf> {
    Some(config_dir()?.join("usb.ids"))
}

struct IdsDb {
    vendors: HashMap<u16, String>,
    products: HashMap<(u16, u16), String>,
}

/// Downloaded usb.ids if present; wins over the compile-time snapshot.
fn cached_ids() -> Option<&'static IdsDb> {
    static CACHE: OnceLock<Option<IdsDb>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let bytes = fs::read(ids_cache_path()?).ok()?;
            Some(parse_usb_ids(&String::from_utf8_lossy(&bytes)))
        })
        .as_ref()
}

fn ids_product(vid: u16, pid: u16) -> Option<String> {
    if let Some(db) = cached_ids()
        && let Some(n) = db.products.get(&(vid, pid))
    {
        return Some(n.clone());
    }
    usb_ids::Device::from_vid_pid(vid, pid).map(|d| d.name().to_string())
}

fn ids_vendor(vid: u16) -> Option<String> {
    if let Some(db) = cached_ids()
        && let Some(n) = db.vendors.get(&vid)
    {
        return Some(n.clone());
    }
    usb_ids::Vendor::from_id(vid).map(|v| v.name().to_string())
}

/// usb.ids format: `vvvv  Vendor`, then tab-indented `pppp  Product` lines.
/// Class/misc sections at the end ("C 03  HID", "AT ...") fail the hex parse
/// and reset the current vendor, so their sub-lines are ignored.
fn parse_usb_ids(s: &str) -> IdsDb {
    let mut vendors = HashMap::new();
    let mut products = HashMap::new();
    let mut cur: Option<u16> = None;
    for line in s.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('\t') {
            if rest.starts_with('\t') {
                continue; // interface / sub-sub entries
            }
            if let (Some(vid), Some((pid, name))) = (cur, split_id_line(rest)) {
                products.insert((vid, pid), name);
            }
        } else if let Some((vid, name)) = split_id_line(line) {
            cur = Some(vid);
            vendors.insert(vid, name);
        } else {
            cur = None;
        }
    }
    IdsDb { vendors, products }
}

fn split_id_line(s: &str) -> Option<(u16, String)> {
    let (id, name) = s.split_once("  ")?;
    Some((u16::from_str_radix(id, 16).ok()?, name.trim().to_string()))
}

/// Download a fresh usb.ids into the config dir. Returns (vendors, products).
pub fn update_list() -> Result<(usize, usize, PathBuf), String> {
    let path = ids_cache_path().ok_or("no config dir found")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // ponytail: shell out to curl (bundled on Win10+, macOS, most Linux)
    // instead of pulling in an HTTP client crate; swap for ureq if it hurts
    let status = std::process::Command::new("curl")
        .args(["-fsSL", "--retry", "2", "-o"])
        .arg(&path)
        .arg(USB_IDS_URL)
        .status()
        .map_err(|e| format!("couldn't run curl: {e}"))?;
    if !status.success() {
        return Err(format!("download failed ({status}) from {USB_IDS_URL}"));
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let db = parse_usb_ids(&String::from_utf8_lossy(&bytes));
    if db.vendors.is_empty() {
        return Err("downloaded file doesn't look like usb.ids".into());
    }
    Ok((db.vendors.len(), db.products.len(), path))
}

fn parse_overrides(s: &str) -> HashMap<(u16, u16), String> {
    s.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (ids, name) = line.split_once(char::is_whitespace)?;
            let (vid, pid) = ids.split_once(':')?;
            Some((
                (
                    u16::from_str_radix(vid, 16).ok()?,
                    u16::from_str_radix(pid, 16).ok()?,
                ),
                name.trim().to_string(),
            ))
        })
        .collect()
}

/// Deterministic fake tree for `--demo` (screenshots, trying the UI without
/// hardware); `t` is seconds since start. On a 30 s loop an SSD plugs in at
/// 6 s and unplugs at 24 s, and a webcam unplugs at 14 s and returns at 20 s.
pub fn demo_scan(t: u64) -> Vec<Device> {
    let dev = |name: &str,
               vid: u16,
               pid: u16,
               manufacturer: &str,
               product: &str,
               speed: &str,
               class: u8,
               ifaces: &[u8]| Device {
        name: name.into(),
        vid,
        pid,
        manufacturer: (!manufacturer.is_empty()).then(|| manufacturer.into()),
        product: Some(product.into()),
        serial: None,
        speed: speed.into(),
        class,
        iface_classes: ifaces.to_vec(),
        devnum: 0,
    };
    let t = t % 30;
    let mut devices = vec![
        dev("usb1", 0, 0, "", "xhci-hcd", "", 0x09, &[]),
        dev("1-1", 0x046d, 0xc52b, "Logitech", "Unifying Receiver", "12", 0x00, &[0x03]),
        dev("1-2", 0x07fd, 0x000b, "MOTU", "M2", "480", 0xef, &[0x01, 0x01, 0xff]),
        dev("1-3", 0x05e3, 0x0610, "Genesys Logic", "USB2.0 Hub", "480", 0x09, &[]),
        dev("1-3.1", 0x3434, 0x0121, "Keychron", "Keychron K8", "12", 0x00, &[0x03]),
        dev("1-3.2", 0x046d, 0xb034, "Logitech", "MX Master 3S", "12", 0x00, &[0x03]),
        dev("usb2", 0, 0, "", "xhci-hcd", "", 0x09, &[]),
    ];
    if (6..24).contains(&t) {
        devices.push(dev(
            "2-1", 0x0781, 0x558c, "SanDisk", "Extreme SSD", "10000", 0x00, &[0x08],
        ));
    }
    if !(14..20).contains(&t) {
        devices.push(dev(
            "2-3", 0x046d, 0x082d, "Logitech", "HD Pro Webcam C920", "480", 0xef, &[0x0e, 0x01],
        ));
    }
    devices.sort_by_key(|d| sort_key(&d.name));
    devices
}

/// Numeric sort: "1-1.10" after "1-1.2", "usb2" after "usb1".
fn sort_key(name: &str) -> Vec<u32> {
    name.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or(0))
        .collect()
}

/// DFS-flatten devices into (depth, index) rows for tree rendering.
/// Children of names in `collapsed` are hidden.
pub fn flatten(devices: &[Device], collapsed: &HashSet<String>) -> Vec<(usize, usize)> {
    let mut rows = Vec::with_capacity(devices.len());
    for (i, d) in devices.iter().enumerate() {
        if d.is_root_hub() {
            push_subtree(devices, i, 0, collapsed, &mut rows);
        }
    }
    // orphans (parent missing from scan) still shown at root level
    let names: HashSet<&str> = devices.iter().map(|d| d.name.as_str()).collect();
    for (i, d) in devices.iter().enumerate() {
        if !d.is_root_hub() && d.parent_name().is_none_or(|p| !names.contains(p.as_str())) {
            push_subtree(devices, i, 0, collapsed, &mut rows);
        }
    }
    rows
}

fn push_subtree(
    devices: &[Device],
    idx: usize,
    depth: usize,
    collapsed: &HashSet<String>,
    rows: &mut Vec<(usize, usize)>,
) {
    rows.push((depth, idx));
    let name = &devices[idx].name;
    if collapsed.contains(name) {
        return;
    }
    for (i, d) in devices.iter().enumerate() {
        if d.parent_name().as_deref() == Some(name) {
            push_subtree(devices, i, depth + 1, collapsed, rows);
        }
    }
}

/// Direct children count (whole subtree below a collapsed node is hidden,
/// but the badge shows immediate children).
pub fn child_count(devices: &[Device], name: &str) -> usize {
    devices
        .iter()
        .filter(|d| d.parent_name().as_deref() == Some(name))
        .count()
}

/// Hot-plug diff: (added, removed) relative to `old`.
pub fn diff<'a>(old: &'a [Device], new: &'a [Device]) -> (Vec<&'a Device>, Vec<&'a Device>) {
    let old_keys: HashSet<String> = old.iter().map(|d| d.key()).collect();
    let new_keys: HashSet<String> = new.iter().map(|d| d.key()).collect();
    let added = new.iter().filter(|d| !old_keys.contains(&d.key())).collect();
    let removed = old.iter().filter(|d| !new_keys.contains(&d.key())).collect();
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, vid: u16, pid: u16, class: u8, ifaces: &[u8]) -> Device {
        Device {
            name: name.into(),
            vid,
            pid,
            manufacturer: None,
            product: None,
            serial: None,
            speed: "480".into(),
            class,
            iface_classes: ifaces.to_vec(),
            devnum: 0,
        }
    }

    #[test]
    fn tree_nests_and_collapses() {
        let devices = vec![
            dev("usb1", 0, 0, 0x09, &[]),
            dev("1-1", 0x046d, 0xc52b, 0x00, &[0x03]),
            dev("1-1.4", 0x05e3, 0x0610, 0x09, &[]),
        ];
        let rows = flatten(&devices, &HashSet::new());
        assert_eq!(
            rows,
            vec![(0, 0), (1, 1), (2, 2)],
            "usb1 > 1-1 > 1-1.4 by depth"
        );
        assert_eq!(devices[1].parent_name().as_deref(), Some("usb1"));
        assert_eq!(devices[2].parent_name().as_deref(), Some("1-1"));
        assert_eq!(
            devices[1].effective_class(),
            0x03,
            "composite falls back to interface class"
        );

        // collapsing usb1 hides the whole subtree, no orphan resurfacing
        let collapsed = HashSet::from(["usb1".to_string()]);
        assert_eq!(flatten(&devices, &collapsed).len(), 1);
        assert_eq!(child_count(&devices, "usb1"), 1);
    }

    #[test]
    fn usb_ids_parse() {
        let sample = "# comment\n\
046d  Logitech, Inc.\n\
\tc52b  Unifying Receiver\n\
\t\t01  weird interface line\n\
07fd  Mark of the Unicorn\n\
\n\
C 03  Human Interface Device\n\
\t01  Boot Interface Subclass\n";
        let db = parse_usb_ids(sample);
        assert_eq!(db.vendors[&0x046d], "Logitech, Inc.");
        assert_eq!(db.products[&(0x046d, 0xc52b)], "Unifying Receiver");
        assert_eq!(db.vendors.len(), 2, "class section must not become a vendor");
        assert_eq!(db.products.len(), 1, "class subclass lines must be ignored");
    }

    #[test]
    fn overrides_parse() {
        let map = parse_overrides("# comment\n07fd:000b MOTU M2 Audio Interface\n\nbad line\n");
        assert_eq!(map.len(), 1);
        assert_eq!(map[&(0x07fd, 0x000b)], "MOTU M2 Audio Interface");
    }

    #[test]
    fn misc_class_resolves_via_interfaces() {
        // Misc/IAD at device level, like the MOTU M2
        let d = dev("1-9", 0x07fd, 0x000b, 0xef, &[0xff, 0x01, 0x01]);
        assert_eq!(d.effective_class(), 0x01);
        assert_eq!(d.class_name(), "Audio");
    }

    #[test]
    fn diff_detects_hotplug() {
        let before = vec![dev("usb1", 0, 0, 0x09, &[]), dev("1-1", 1, 2, 0x03, &[])];
        let mut after = before.clone();
        after.push(dev("1-2", 0x0781, 0x5567, 0x08, &[]));
        let (added, removed) = diff(&before, &after);
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].name, "1-2");
        assert!(removed.is_empty());
        let (added, removed) = diff(&after, &before);
        assert_eq!(removed.len(), 1);
        assert!(added.is_empty());
    }
}
