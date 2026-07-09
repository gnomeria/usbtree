// ponytail: SPIKE. Proves pci-info enumerates cross-platform + names resolve,
// rendered as a bus-grouped tree via --pci. Not wired into the TUI yet — PCI has
// its own class numbering (0x01 = storage, not USB's 0x08), no per-device traffic
// counters, and no hotplug, so it can't reuse usb::Device or the live App as-is.
// Promote to a TUI tab only once this shape is agreed.

use pci_ids::FromId;
use pci_info::PciInfo;

pub struct PciDevice {
    /// PCI address, "segment:bus:device.function" e.g. "0000:00:1f.2"
    pub addr: String,
    pub vid: u16,
    pub pid: u16,
    pub class: u8,
    pub subclass: u8,
    /// `prog-if`: programming interface byte. 0 where unknown.
    pub prog_if: u8,
    pub revision: u8,
    /// subsystem (vendor, device) ids, when the backend exposes them.
    pub subsystem: Option<(u16, u16)>,
    /// bound kernel driver (Linux only; None elsewhere).
    pub driver: Option<String>,
    /// negotiated PCIe link (Linux sysfs; None on non-PCIe or other platforms).
    pub link: Option<PciLink>,
    /// NUMA node id (Linux sysfs; None if single-socket / other platforms).
    pub numa_node: Option<u16>,
    /// IOMMU group id (Linux sysfs; None if IOMMU off / other platforms).
    pub iommu_group: Option<String>,
    /// ACPI power state e.g. "D0" (Linux sysfs; None elsewhere).
    pub power_state: Option<String>,
}

/// Negotiated vs. maximum PCIe link (Linux sysfs). Speeds are the cleaned sysfs
/// strings, e.g. "8.0 GT/s"; widths are lane counts.
pub struct PciLink {
    pub cur_speed: String,
    pub cur_width: u8,
    pub max_speed: String,
    pub max_width: u8,
}

impl PciLink {
    /// True when the slot negotiated below its own maximum (fewer lanes or a
    /// slower speed) — a common laptop/riser bottleneck worth flagging.
    pub fn throttled(&self) -> bool {
        self.max_width > 0 && (self.cur_width < self.max_width || self.cur_speed != self.max_speed)
    }
}

impl PciDevice {
    /// "segment:bus" group key, e.g. "0000:00" — the tree's first level.
    pub fn bus(&self) -> &str {
        self.addr.rsplit_once(':').map_or(self.addr.as_str(), |(b, _)| b)
    }

    pub fn vendor_name(&self) -> String {
        pci_ids::Vendor::from_id(self.vid)
            .map(|v| v.name().to_string())
            .unwrap_or_else(|| format!("{:04x}", self.vid))
    }

    /// Broad class name only (no subclass), for the fixed-width tree gutter.
    pub fn class_group(&self) -> &'static str {
        pci_ids::Class::from_id(self.class).map_or("Unknown", pci_ids::Class::name)
    }

    pub fn label(&self) -> String {
        match pci_ids::Device::from_vid_pid(self.vid, self.pid) {
            Some(d) => d.name().to_string(),
            None => match pci_ids::Vendor::from_id(self.vid) {
                Some(v) => format!("{} {}", v.name(), self.class_name()),
                None => format!("Unknown device {:04x}:{:04x}", self.vid, self.pid),
            },
        }
    }

    /// True if `q` (already lowercased) matches any human-facing field.
    pub fn matches(&self, q: &str) -> bool {
        self.addr.contains(q)
            || self.label().to_lowercase().contains(q)
            || self.vendor_name().to_lowercase().contains(q)
            || self.class_name().to_lowercase().contains(q)
            || format!("{:04x}:{:04x}", self.vid, self.pid).contains(q)
    }

    /// `prog-if` name (e.g. "NVM Express", "XHCI") when the DB has one.
    pub fn prog_if_name(&self) -> Option<&'static str> {
        let name = pci_ids::Class::from_id(self.class)?
            .subclasses()
            .find(|s| s.id() == self.subclass)?
            .prog_ifs()
            .find(|p| p.id() == self.prog_if)?
            .name();
        (!name.is_empty()).then_some(name)
    }

    /// Human name for the add-in board (subsystem): the exact subsystem device
    /// name if the DB has the pair, else the subsystem vendor. None if unknown.
    pub fn subsystem_name(&self) -> Option<String> {
        let (sv, sd) = self.subsystem?;
        if let Some(dev) = pci_ids::Device::from_vid_pid(self.vid, self.pid)
            && let Some(ss) = dev
                .subsystems()
                .find(|s| s.subvendor() == sv && s.subdevice() == sd)
        {
            return Some(ss.name().to_string());
        }
        pci_ids::Vendor::from_id(sv).map(|v| v.name().to_string())
    }

    /// Subclass name if the DB has one, else the broad class name.
    pub fn class_name(&self) -> &'static str {
        if let Some(c) = pci_ids::Class::from_id(self.class) {
            if let Some(s) = c.subclasses().find(|s| s.id() == self.subclass) {
                return s.name();
            }
            return c.name();
        }
        "Unknown class"
    }
}

/// Enumerate PCI devices via pci-info (Linux procfs, macOS IOKit, Windows
/// SetupAPI, FreeBSD). Unprivileged. Missing per-device properties are skipped,
/// not fatal — a partial device still lists its vid:pid.
pub fn scan() -> Vec<PciDevice> {
    let Ok(info) = PciInfo::enumerate_pci() else {
        return Vec::new();
    };
    let mut out: Vec<PciDevice> = info
        .into_iter()
        .filter_map(Result::ok)
        .map(|d| PciDevice {
            addr: d.location().map(|l| l.to_string()).unwrap_or_default(),
            vid: d.vendor_id(),
            pid: d.device_id(),
            class: d.device_class_code().unwrap_or(0),
            subclass: d.device_subclass_code().unwrap_or(0),
            prog_if: d.device_iface_code().unwrap_or(0),
            revision: d.revision().unwrap_or(0),
            subsystem: match (d.subsystem_vendor_id(), d.subsystem_device_id()) {
                (Ok(Some(v)), Ok(Some(p))) => Some((v, p)),
                _ => None,
            },
            driver: d.os_driver().ok().and_then(|o| o.clone()),
            link: None,
            numa_node: None,
            iommu_group: None,
            power_state: None,
        })
        .collect();
    #[cfg(target_os = "linux")]
    for d in &mut out {
        d.read_sysfs();
    }
    out.sort_by(|a, b| a.addr.cmp(&b.addr));
    out
}

/// Read one sysfs attribute for a PCI device, trimmed; None if missing/empty.
#[cfg(target_os = "linux")]
fn sysfs_read(addr: &str, attr: &str) -> Option<String> {
    let s = std::fs::read_to_string(format!("/sys/bus/pci/devices/{addr}/{attr}")).ok()?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

#[cfg(target_os = "linux")]
impl PciDevice {
    /// Fill the Linux-only fields (PCIe link, NUMA, IOMMU, power) from sysfs.
    fn read_sysfs(&mut self) {
        // "8.0 GT/s PCIe" → "8.0 GT/s"; drops the redundant bus suffix.
        let speed = |a| sysfs_read(&self.addr, a).map(|s| s.trim_end_matches(" PCIe").to_string());
        let width = |a| sysfs_read(&self.addr, a).and_then(|s| s.parse().ok());
        // A live PCIe link needs at least a parseable current speed + width.
        if let (Some(cur_speed), Some(cur_width)) =
            (speed("current_link_speed"), width("current_link_width"))
        {
            self.link = Some(PciLink {
                cur_speed,
                cur_width,
                max_speed: speed("max_link_speed").unwrap_or_default(),
                max_width: width("max_link_width").unwrap_or(0),
            });
        }
        self.numa_node = sysfs_read(&self.addr, "numa_node")
            .and_then(|s| s.parse::<i32>().ok())
            .filter(|&n| n >= 0)
            .map(|n| n as u16);
        self.iommu_group = std::fs::read_link(format!("/sys/bus/pci/devices/{}/iommu_group", self.addr))
            .ok()
            .and_then(|p| p.file_name()?.to_str().map(String::from));
        self.power_state = sysfs_read(&self.addr, "power_state");
    }
}

/// Deterministic fake PCI list for `--demo` (screenshots, no hardware). Mirrors
/// a typical x86 laptop: root/bridges, GPU, NVMe, NIC, wifi, USB controller.
pub fn demo_scan() -> Vec<PciDevice> {
    let dev = |addr: &str, vid, pid, class, subclass, prog_if, driver: &str| PciDevice {
        addr: addr.into(),
        vid,
        pid,
        class,
        subclass,
        prog_if,
        revision: 0x01,
        subsystem: Some((vid, pid)),
        driver: (!driver.is_empty()).then(|| driver.into()),
        link: None,
        numa_node: None,
        iommu_group: None,
        power_state: None,
    };
    let link = |cw, cs: &str, mw, ms: &str| PciLink {
        cur_width: cw,
        cur_speed: cs.into(),
        max_width: mw,
        max_speed: ms.into(),
    };
    let mut devices = vec![
        dev("0000:00:00.0", 0x8086, 0x9b61, 0x06, 0x00, 0x00, "skl_uncore"),
        dev("0000:00:02.0", 0x8086, 0x9bc4, 0x03, 0x00, 0x00, "i915"),
        dev("0000:00:14.0", 0x8086, 0x06ed, 0x0c, 0x03, 0x30, "xhci_hcd"),
        dev("0000:00:1f.0", 0x8086, 0x0685, 0x06, 0x01, 0x00, ""),
        dev("0000:01:00.0", 0x10de, 0x1f95, 0x03, 0x00, 0x00, "nvidia"),
        dev("0000:02:00.0", 0x144d, 0xa808, 0x01, 0x08, 0x02, "nvme"),
        dev("0000:03:00.0", 0x8086, 0x2723, 0x02, 0x80, 0x00, "iwlwifi"),
        dev("0000:04:00.0", 0x10ec, 0x8168, 0x02, 0x00, 0x00, "r8169"),
    ];
    // Synthetic link/power so the detail pane isn't blank off-Linux. The GPU
    // rides a shared x8 muxer while its endpoint is x16 → shows the throttle flag.
    for d in &mut devices {
        d.power_state = Some("D0".into());
        if d.bus() == "0000:00" {
            continue; // on-package (iGPU, bridges, xHCI): no discrete PCIe link
        }
        d.link = match d.class {
            0x03 => Some(link(8, "8.0 GT/s", 16, "16.0 GT/s")), // GPU, x8 of x16
            0x01 => Some(link(4, "8.0 GT/s", 4, "8.0 GT/s")),   // NVMe, full x4
            0x02 | 0x0d => Some(link(1, "5.0 GT/s", 1, "5.0 GT/s")),
            _ => None,
        };
    }
    devices[4].iommu_group = Some("13".into()); // discrete GPU, its own group
    devices[5].iommu_group = Some("14".into()); // NVMe
    devices.sort_by(|a, b| a.addr.cmp(&b.addr));
    devices
}

/// Print the bus-grouped tree once and exit (`--pci`). Standalone so the TUI
/// stays USB-only until the tab lands.
pub fn dump() {
    let devices = scan();
    if devices.is_empty() {
        eprintln!("no PCI devices found (backend may be unsupported on this platform)");
        return;
    }
    let mut cur_bus = "";
    for d in &devices {
        if d.bus() != cur_bus {
            cur_bus = d.bus();
            println!("{cur_bus}");
        }
        println!(
            "  {} {:04x}:{:04x} [{}] {}",
            d.addr,
            d.vid,
            d.pid,
            d.class_name(),
            d.label()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_key_strips_device_function() {
        let d = PciDevice {
            addr: "0000:00:1f.2".into(),
            vid: 0,
            pid: 0,
            class: 0,
            subclass: 0,
            prog_if: 0,
            revision: 0,
            subsystem: None,
            driver: None,
            link: None,
            numa_node: None,
            iommu_group: None,
            power_state: None,
        };
        assert_eq!(d.bus(), "0000:00");
    }
}
