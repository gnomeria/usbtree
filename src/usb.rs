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
    /// `bMaxPower`: max current the device *requests* from the bus, in mA —
    /// advertised, not measured draw. Linux (sysfs) + macOS (config descriptor);
    /// None on Windows.
    pub max_power_ma: Option<u16>,
    /// Interfaces + endpoints of the first configuration (`lsusb -v` depth).
    /// Linux: unprivileged sysfs `descriptors`. macOS: from the config open we
    /// already do for power. Empty on Windows (no descriptor read there yet).
    pub interfaces: Vec<Interface>,
}

/// One interface alternate setting and its endpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct Interface {
    pub number: u8,
    pub alt: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub endpoints: Vec<Endpoint>,
}

/// One endpoint descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub address: u8,
    /// true = IN (device→host), from `bEndpointAddress` bit 7.
    pub input: bool,
    /// transfer type: 0 control, 1 iso, 2 bulk, 3 interrupt.
    pub transfer: u8,
    pub max_packet: u16,
    pub interval: u8,
}

/// Parse interfaces + endpoints from a raw config-descriptor buffer. Accepts a
/// bare configuration descriptor (macOS `active_configuration`) or a Linux
/// sysfs `descriptors` blob that leads with the 18-byte device descriptor.
// ponytail: reads only the first configuration; multi-config devices are rare
// and the active one is almost always first. Add config selection if asked.
#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn parse_interfaces(blob: &[u8]) -> Vec<Interface> {
    let cfg_bytes = match (blob.first(), blob.get(1)) {
        (Some(18), Some(1)) => &blob[18..], // skip leading device descriptor
        _ => blob,
    };
    let Some(cfg) = nusb::descriptors::ConfigurationDescriptor::new(cfg_bytes) else {
        return Vec::new();
    };
    cfg.interface_alt_settings()
        .map(|intf| Interface {
            number: intf.interface_number(),
            alt: intf.alternate_setting(),
            class: intf.class(),
            subclass: intf.subclass(),
            protocol: intf.protocol(),
            endpoints: intf
                .endpoints()
                .map(|e| Endpoint {
                    address: e.address(),
                    input: e.address() & 0x80 != 0,
                    transfer: e.attributes() & 0x03,
                    max_packet: e.max_packet_size() as u16,
                    interval: e.interval(),
                })
                .collect(),
        })
        .collect()
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
    /// Only the Linux usbmon path keys on it; dead elsewhere.
    #[cfg(target_os = "linux")]
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
    // Windows bus ids are opaque PnP paths ("PCIROOT(0)#...#USBROOT(0)"), not
    // small integers — map each distinct id to a clean bus number so names read
    // "usb1"/"1-16", not the raw path. Same id -> same label keeps hub and its
    // devices linked via parent_name().
    let mut labels: HashMap<String, String> = HashMap::new();
    let mut next_bus = 1u32;
    if let Ok(buses) = nusb::list_buses().wait() {
        for bus in buses {
            let label = labels
                .entry(bus.bus_id().to_string())
                .or_insert_with(|| bus_label(bus.bus_id(), &mut next_bus))
                .clone();
            devices.push(Device {
                name: format!("usb{label}"),
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
                max_power_ma: None,
                interfaces: Vec::new(),
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
        let (max_power_ma, interfaces) = descriptors(&d);
        let label = labels
            .get(d.bus_id())
            .cloned()
            .unwrap_or_else(|| bus_label(d.bus_id(), &mut next_bus));
        devices.push(Device {
            name: format!("{label}-{}", path.join(".")),
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
            max_power_ma,
            interfaces,
        });
    }
    devices.sort_by_key(|d| sort_key(&d.name));
    devices
}

/// `bMaxPower` + interfaces on Linux: both from unprivileged sysfs, no device
/// open. Power from the `bMaxPower` attribute, interfaces from the raw
/// `descriptors` blob (device + config descriptors concatenated).
#[cfg(target_os = "linux")]
fn descriptors(d: &nusb::DeviceInfo) -> (Option<u16>, Vec<Interface>) {
    let sysfs = d.sysfs_path();
    let power = fs::read_to_string(sysfs.join("bMaxPower"))
        .ok()
        .and_then(|s| parse_max_power(&s));
    let ifaces = fs::read(sysfs.join("descriptors"))
        .map(|b| parse_interfaces(&b))
        .unwrap_or_default();
    (power, ifaces)
}

/// Parse a sysfs `bMaxPower` value like "500mA" -> 500.
#[cfg(any(target_os = "linux", test))]
fn parse_max_power(s: &str) -> Option<u16> {
    s.trim().trim_end_matches("mA").trim().parse().ok()
}

/// `bMaxPower` + interfaces on macOS: no sysfs, so open the device
/// (unprivileged — works even while mounted) and read its active config
/// descriptor once. Power: the raw byte is 2mA units, or 8mA for SuperSpeed.
/// Self-powered devices legitimately report 0.
///
/// Cached by locationID: descriptors never change for a plugged device, so we
/// open each one once instead of on every 1s rescan.
// ponytail: cache is per-session, never evicted, and a transient open failure
// sticks until replug (new locationID). Both fine at this scale — upgrade to a
// TTL only if devices start flapping their descriptors.
#[cfg(target_os = "macos")]
type DescCache = std::sync::Mutex<HashMap<u32, (Option<u16>, Vec<Interface>)>>;

#[cfg(target_os = "macos")]
fn descriptors(d: &nusb::DeviceInfo) -> (Option<u16>, Vec<Interface>) {
    static CACHE: OnceLock<DescCache> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let loc = d.location_id();
    if let Some(v) = cache.lock().unwrap().get(&loc) {
        return v.clone();
    }
    let unit = match d.speed() {
        Some(nusb::Speed::Super | nusb::Speed::SuperPlus) => 8,
        _ => 2,
    };
    let v = d
        .open()
        .wait()
        .ok()
        .and_then(|dev| {
            let c = dev.active_configuration().ok()?;
            Some((Some(u16::from(c.max_power()) * unit), parse_interfaces(c.as_bytes())))
        })
        .unwrap_or((None, Vec::new()));
    cache.lock().unwrap().insert(loc, v.clone());
    v
}

/// Windows exposes no unprivileged descriptor read here yet.
// ponytail: WinUSB can supply these via open(), but that's a behavior change on
// a platform that currently opens nothing — wire it up when someone needs it.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn descriptors(_d: &nusb::DeviceInfo) -> (Option<u16>, Vec<Interface>) {
    (None, Vec::new())
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

/// Clean display bus number for a raw nusb bus id. Linux/macOS ids are already
/// numeric ("001" -> "1"). Windows ids are opaque PnP location paths; those get
/// a synthesized sequential number (`next`) so the tree stays compact and hubs
/// on separate controllers never collide.
// ponytail: a real platform is homogeneous (all-numeric or all-path), so synth
// numbers can't collide with a passthrough number. Mixed ids would need next to
// start past the max numeric id.
fn bus_label(raw: &str, next: &mut u32) -> String {
    let t = tidy_bus(raw);
    if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) {
        return t.to_string();
    }
    let n = *next;
    *next += 1;
    n.to_string()
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

// ---- release update check ----

/// Newest release tag on GitHub, without the leading `v` (e.g. `0.0.2`), or
/// None on any failure (offline, no curl, no releases yet). We hit the
/// `releases/latest` redirect and read the final URL's tag — no JSON parse,
/// no API token, no User-Agent quirks.
pub fn latest_release() -> Option<String> {
    let url = format!("https://github.com/{}/releases/latest", env!("CARGO_PKG_REPOSITORY").rsplit("github.com/").next()?);
    let out = std::process::Command::new("curl")
        .args(["-fsSLI", "--max-time", "5", "-o", "/dev/null", "-w", "%{url_effective}"])
        .arg(&url)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let final_url = String::from_utf8_lossy(&out.stdout);
    let tag = final_url.trim().rsplit('/').next()?;
    let ver = tag.strip_prefix('v').unwrap_or(tag);
    // when no release exists the redirect lands on `.../releases`, not a tag
    ver.chars().next()?.is_ascii_digit().then(|| ver.to_string())
}

/// True if release `latest` is strictly newer than `current` by numeric
/// major.minor.patch. Non-numeric or missing parts count as 0.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parts = |s: &str| -> Vec<u64> {
        s.split('.').map(|p| p.parse().unwrap_or(0)).collect()
    };
    let (l, c) = (parts(latest), parts(current));
    for i in 0..l.len().max(c.len()) {
        let (a, b) = (l.get(i).copied().unwrap_or(0), c.get(i).copied().unwrap_or(0));
        if a != b {
            return a > b;
        }
    }
    false
}

// ---- downloadable usb.ids (usbtree --updatelist) ----

const USB_IDS_URL: &str = "https://raw.githubusercontent.com/vcrhonek/hwdata/master/usb.ids";

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
               ifaces: &[u8],
               power: Option<u16>| Device {
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
        max_power_ma: power,
        interfaces: demo_interfaces(name),
    };
    let t = t % 30;
    let mut devices = vec![
        dev("usb1", 0, 0, "", "xhci-hcd", "", 0x09, &[], None),
        dev("1-1", 0x046d, 0xc52b, "Logitech", "Unifying Receiver", "12", 0x00, &[0x03], Some(98)),
        dev("1-2", 0x07fd, 0x000b, "MOTU", "M2", "480", 0xef, &[0x01, 0x01, 0xff], Some(500)),
        dev("1-3", 0x05e3, 0x0610, "Genesys Logic", "USB2.0 Hub", "480", 0x09, &[], Some(100)),
        dev("1-3.1", 0x3434, 0x0121, "Keychron", "Keychron K8", "12", 0x00, &[0x03], Some(500)),
        dev("1-3.2", 0x046d, 0xb034, "Logitech", "MX Master 3S", "12", 0x00, &[0x03], Some(100)),
        dev("usb2", 0, 0, "", "xhci-hcd", "", 0x09, &[], None),
    ];
    if (6..24).contains(&t) {
        devices.push(dev(
            "2-1", 0x0781, 0x558c, "SanDisk", "Extreme SSD", "10000", 0x00, &[0x08], Some(896),
        ));
    }
    if !(14..20).contains(&t) {
        devices.push(dev(
            "2-3", 0x046d, 0x082d, "Logitech", "HD Pro Webcam C920", "480", 0xef, &[0x0e, 0x01],
            Some(500),
        ));
    }
    devices.sort_by_key(|d| sort_key(&d.name));
    devices
}

/// Representative interfaces/endpoints for `--demo` devices, so the detail
/// panel and screenshots show real descriptor depth without hardware.
fn demo_interfaces(name: &str) -> Vec<Interface> {
    let ep = |address: u8, transfer: u8, max_packet: u16, interval: u8| Endpoint {
        address,
        input: address & 0x80 != 0,
        transfer,
        max_packet,
        interval,
    };
    let iface = |number, class, subclass, protocol, endpoints| Interface {
        number,
        alt: 0,
        class,
        subclass,
        protocol,
        endpoints,
    };
    match name {
        "1-1" => vec![
            iface(0, 0x03, 0x01, 0x01, vec![ep(0x81, 3, 8, 8)]),
            iface(1, 0x03, 0x00, 0x00, vec![ep(0x82, 3, 32, 2)]),
        ],
        "1-2" => vec![
            iface(0, 0x01, 0x01, 0x00, vec![]),
            iface(1, 0x01, 0x02, 0x00, vec![ep(0x01, 1, 294, 1)]),
            iface(2, 0x01, 0x02, 0x00, vec![ep(0x82, 1, 294, 1)]),
            iface(3, 0xff, 0x00, 0x00, vec![ep(0x83, 3, 4, 8)]),
        ],
        "1-3.1" => vec![
            iface(0, 0x03, 0x01, 0x01, vec![ep(0x81, 3, 8, 10)]),
            iface(1, 0x03, 0x00, 0x00, vec![ep(0x82, 3, 16, 10)]),
        ],
        "1-3.2" => vec![iface(0, 0x03, 0x01, 0x02, vec![ep(0x81, 3, 8, 8)])],
        "2-1" => vec![iface(
            0,
            0x08,
            0x06,
            0x50,
            vec![ep(0x81, 2, 1024, 0), ep(0x02, 2, 1024, 0)],
        )],
        "2-3" => vec![
            iface(0, 0x0e, 0x01, 0x00, vec![ep(0x87, 3, 16, 8)]),
            iface(1, 0x0e, 0x02, 0x00, vec![ep(0x81, 1, 3072, 1)]),
            iface(2, 0x01, 0x01, 0x00, vec![]),
            iface(3, 0x01, 0x02, 0x00, vec![ep(0x86, 1, 192, 4)]),
        ],
        _ => Vec::new(),
    }
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
            max_power_ma: None,
            interfaces: Vec::new(),
        }
    }

    #[test]
    fn max_power_parses_sysfs() {
        assert_eq!(parse_max_power("500mA\n"), Some(500));
        assert_eq!(parse_max_power("0mA"), Some(0));
        assert_eq!(parse_max_power(""), None);
    }

    #[test]
    fn parses_interfaces_and_endpoints() {
        // config(9) + interface(9, mass-storage BOT) + endpoint(7, bulk IN 512)
        #[rustfmt::skip]
        let config = [
            9, 2, 25, 0, 1, 1, 0, 0x80, 50,
            9, 4, 0, 0, 1, 0x08, 0x06, 0x50, 0,
            7, 5, 0x81, 0x02, 0x00, 0x02, 0,
        ];
        let ifaces = parse_interfaces(&config);
        assert_eq!(ifaces.len(), 1);
        let i = &ifaces[0];
        assert_eq!((i.class, i.subclass, i.protocol), (0x08, 0x06, 0x50));
        assert_eq!(i.endpoints.len(), 1);
        let e = &i.endpoints[0];
        assert_eq!(e.address, 0x81);
        assert!(e.input);
        assert_eq!(e.transfer, 2); // bulk
        assert_eq!(e.max_packet, 512);

        // Linux sysfs blob leads with an 18-byte device descriptor; skip it.
        let mut sysfs = vec![18, 1];
        sysfs.extend(std::iter::repeat_n(0, 16));
        sysfs.extend_from_slice(&config);
        assert_eq!(parse_interfaces(&sysfs), ifaces);
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

    #[test]
    fn bus_label_passes_numeric_synths_windows_paths() {
        let mut n = 1;
        assert_eq!(bus_label("001", &mut n), "1"); // Linux
        assert_eq!(bus_label("42", &mut n), "42"); // macOS
        // Windows opaque paths -> sequential synth, distinct paths distinct nums
        assert_eq!(bus_label("PCIROOT(0)#PCI(0201)#PCI(0000)#USBROOT(0)", &mut n), "1");
        assert_eq!(bus_label("PCIROOT(0)#PCI(0201)#PCI(0000)#USBROOT(1)", &mut n), "2");
        assert_eq!(n, 3);
        // synth names still link child -> hub
        assert_eq!(dev("1-16", 0, 0, 0x03, &[]).parent_name().as_deref(), Some("usb1"));
    }

    #[test]
    fn version_compare() {
        assert!(is_newer("0.0.2", "0.0.1"));
        assert!(is_newer("0.1.0", "0.0.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.0.1", "0.0.1"));
        assert!(!is_newer("0.0.1", "0.0.2"));
        assert!(is_newer("0.0.2", "0.0.1")); // shorter/longer parts default to 0
    }
}
