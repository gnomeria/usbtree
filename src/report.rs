use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::usb::{self, Device};

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotDevice {
    pub name: String,
    pub vid: String,
    pub pid: String,
    pub label: String,
    pub class_name: String,
    pub speed: String,
}

impl SnapshotDevice {
    fn key(&self) -> String {
        format!("{} {}:{}", self.name, self.vid, self.pid)
    }
}

pub fn json(devices: &[Device]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"format\": \"usbtree.snapshot.v1\",\n");
    out.push_str(&format!("  \"generated_at_unix\": {},\n", unix_now()));
    out.push_str("  \"devices\": [\n");
    for (i, d) in devices.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        write_device_json(&mut out, devices, d, 4);
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

pub fn markdown(devices: &[Device]) -> String {
    let mut out = String::new();
    out.push_str("# usbtree report\n\n");
    out.push_str("| Path | Device | Class | Speed | Power | Hints |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for d in devices {
        let hints = usb::troubleshooting_hints(devices, d).join("; ");
        let power = d
            .max_power_ma
            .map(|ma| format!("{ma} mA"))
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "| `{}` | {} `{}` | {} | {} | {} | {} |\n",
            escape_md(&d.name),
            escape_md(&d.label()),
            vid_pid(d),
            d.class_name(),
            if d.speed.is_empty() { "-" } else { &d.speed },
            power,
            if hints.is_empty() { "-".to_string() } else { escape_md(&hints) },
        ));
    }
    out.push('\n');
    for d in devices {
        out.push_str(&format!("## {} {}\n\n", d.name, escape_md(&d.label())));
        if let Some(root) = usb::root_hub_for(devices, d) {
            out.push_str(&format!(
                "- Controller: {}{}\n",
                root.product.as_deref().unwrap_or(&root.name),
                root.manufacturer
                    .as_ref()
                    .map(|m| format!(" ({m})"))
                    .unwrap_or_default()
            ));
        }
        if let Some(platform) = &d.platform {
            out.push_str(&format!("- Platform: {}\n", escape_md(platform)));
        }
        let chain = usb::hub_chain(devices, d);
        if !chain.is_empty() {
            out.push_str(&format!(
                "- Hub chain: {}\n",
                chain
                    .iter()
                    .map(|hub| hub.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" > ")
            ));
        }
        out.push_str(&format!("- VID:PID: `{}`\n", vid_pid(d)));
        out.push_str(&format!("- Class: {} (`{:02x}:{:02x}:{:02x}`)\n", d.class_name(), d.class, d.subclass, d.protocol));
        if d.usb_version != 0 {
            out.push_str(&format!("- USB spec: {}\n", usb::bcd_version(d.usb_version)));
        }
        if d.config_attributes.is_some() || d.max_power_ma.is_some() {
            out.push_str("- Configuration 1: active");
            if d.is_self_powered() {
                out.push_str(", self-powered");
            } else if d.is_bus_powered() {
                out.push_str(", bus-powered");
            }
            if d.config_attributes.is_some_and(|a| a & 0x20 != 0) {
                out.push_str(", remote-wakeup");
            }
            if let Some(ma) = d.max_power_ma {
                out.push_str(&format!(", {ma} mA max"));
            }
            out.push('\n');
        }
        if !d.interfaces.is_empty() {
            out.push_str("\n| Interface | Class | Endpoint | Direction | Transfer | Max packet | Interval |\n");
            out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
            for iface in &d.interfaces {
                if iface.endpoints.is_empty() {
                    out.push_str(&format!(
                        "| {} | {} `{:02x}:{:02x}:{:02x}` | - | - | - | - | - |\n",
                        iface.number,
                        usb::class_name(iface.class),
                        iface.class,
                        iface.subclass,
                        iface.protocol
                    ));
                }
                for ep in &iface.endpoints {
                    out.push_str(&format!(
                        "| {} | {} `{:02x}:{:02x}:{:02x}` | `{:#04x}` | {} | {} | {} B | {} |\n",
                        iface.number,
                        usb::class_name(iface.class),
                        iface.class,
                        iface.subclass,
                        iface.protocol,
                        ep.address,
                        if ep.input { "IN" } else { "OUT" },
                        usb::transfer_type_name(ep.transfer),
                        ep.max_packet,
                        if ep.interval == 0 { "-".to_string() } else { ep.interval.to_string() }
                    ));
                }
            }
        }
        let hints = usb::troubleshooting_hints(devices, d);
        if !hints.is_empty() {
            out.push_str("\nHints:\n");
            for hint in hints {
                out.push_str(&format!("- {}\n", escape_md(&hint)));
            }
        }
        out.push('\n');
    }
    out
}

pub fn parse_snapshot_devices(input: &str) -> Result<Vec<SnapshotDevice>, String> {
    let start = input
        .find("\"devices\"")
        .ok_or_else(|| "snapshot missing devices array".to_string())?;
    let array_start = input[start..]
        .find('[')
        .map(|i| start + i)
        .ok_or_else(|| "snapshot missing devices array".to_string())?;
    let array_end = matching_bracket(input, array_start, '[', ']')
        .ok_or_else(|| "snapshot devices array is malformed".to_string())?;
    let mut devices = Vec::new();
    let mut i = array_start + 1;
    while i < array_end {
        let Some(rel) = input[i..array_end].find('{') else {
            break;
        };
        let obj_start = i + rel;
        let obj_end = matching_bracket(input, obj_start, '{', '}')
            .ok_or_else(|| "snapshot device object is malformed".to_string())?;
        let object = &input[obj_start..=obj_end];
        devices.push(SnapshotDevice {
            name: json_field(object, "name").ok_or_else(|| "device missing name".to_string())?,
            vid: json_field(object, "vid").ok_or_else(|| "device missing vid".to_string())?,
            pid: json_field(object, "pid").ok_or_else(|| "device missing pid".to_string())?,
            label: json_field(object, "label").unwrap_or_default(),
            class_name: json_field(object, "class_name").unwrap_or_default(),
            speed: json_field(object, "speed_mbps").unwrap_or_default(),
        });
        i = obj_end + 1;
    }
    Ok(devices)
}

pub fn diff(previous: &[SnapshotDevice], current: &[Device]) -> String {
    let prev: HashMap<_, _> = previous.iter().map(|d| (d.key(), d)).collect();
    let curr_snapshot = current
        .iter()
        .map(|d| SnapshotDevice {
            name: d.name.clone(),
            vid: format!("{:04x}", d.vid),
            pid: format!("{:04x}", d.pid),
            label: d.label(),
            class_name: d.class_name().to_string(),
            speed: d.speed.clone(),
        })
        .collect::<Vec<_>>();
    let curr: HashMap<_, _> = curr_snapshot.iter().map(|d| (d.key(), d)).collect();
    let mut out = String::new();
    let mut added = curr
        .iter()
        .filter(|(key, _)| !prev.contains_key(*key))
        .map(|(_, d)| *d)
        .collect::<Vec<_>>();
    let mut removed = prev
        .iter()
        .filter(|(key, _)| !curr.contains_key(*key))
        .map(|(_, d)| *d)
        .collect::<Vec<_>>();
    let mut changed = curr
        .iter()
        .filter_map(|(key, d)| {
            let p = prev.get(key)?;
            (p.label != d.label || p.class_name != d.class_name || p.speed != d.speed).then_some((*p, *d))
        })
        .collect::<Vec<_>>();
    added.sort_by(|a, b| a.name.cmp(&b.name));
    removed.sort_by(|a, b| a.name.cmp(&b.name));
    changed.sort_by(|a, b| a.1.name.cmp(&b.1.name));

    out.push_str(&format!(
        "added: {}, removed: {}, changed: {}\n",
        added.len(),
        removed.len(),
        changed.len()
    ));
    for d in added {
        out.push_str(&format!("+ {} {}:{} {}\n", d.name, d.vid, d.pid, d.label));
    }
    for d in removed {
        out.push_str(&format!("- {} {}:{} {}\n", d.name, d.vid, d.pid, d.label));
    }
    for (before, after) in changed {
        out.push_str(&format!("~ {} {}:{}\n", after.name, after.vid, after.pid));
        if before.label != after.label {
            out.push_str(&format!("  label: {} -> {}\n", before.label, after.label));
        }
        if before.class_name != after.class_name {
            out.push_str(&format!("  class: {} -> {}\n", before.class_name, after.class_name));
        }
        if before.speed != after.speed {
            out.push_str(&format!("  speed: {} -> {}\n", before.speed, after.speed));
        }
    }
    out
}

fn write_device_json(out: &mut String, devices: &[Device], d: &Device, indent: usize) {
    let pad = " ".repeat(indent);
    let pad2 = " ".repeat(indent + 2);
    out.push_str(&format!("{pad}{{\n"));
    field(out, indent + 2, "name", &d.name, true);
    field(out, indent + 2, "parent", &d.parent_name().unwrap_or_default(), true);
    field(out, indent + 2, "label", &d.label(), true);
    field(out, indent + 2, "vendor", &d.vendor_name(), true);
    field(out, indent + 2, "vid", &format!("{:04x}", d.vid), true);
    field(out, indent + 2, "pid", &format!("{:04x}", d.pid), true);
    field(out, indent + 2, "class_name", d.class_name(), true);
    out.push_str(&format!("{pad2}\"class_code\": \"{:02x}:{:02x}:{:02x}\",\n", d.class, d.subclass, d.protocol));
    field(out, indent + 2, "speed_mbps", &d.speed, true);
    out.push_str(&format!("{pad2}\"usb_version\": \"{}\",\n", if d.usb_version == 0 { String::new() } else { usb::bcd_version(d.usb_version) }));
    opt_field(out, indent + 2, "serial", d.serial.as_deref(), true);
    opt_field(out, indent + 2, "platform", d.platform.as_deref(), true);
    let chain = usb::hub_chain(devices, d)
        .iter()
        .map(|hub| hub.name.clone())
        .collect::<Vec<_>>();
    array_field(out, indent + 2, "hub_chain", &chain, true);
    let hints = usb::troubleshooting_hints(devices, d);
    array_field(out, indent + 2, "hints", &hints, true);
    out.push_str(&format!("{pad2}\"configurations\": [\n"));
    out.push_str(&format!("{pad2}  {{\n"));
    out.push_str(&format!("{pad2}    \"index\": 1,\n"));
    out.push_str(&format!("{pad2}    \"active\": true,\n"));
    opt_number_field(out, indent + 6, "attributes", d.config_attributes.map(u64::from), true);
    opt_number_field(out, indent + 6, "max_power_ma", d.max_power_ma.map(u64::from), true);
    out.push_str(&format!("{pad2}    \"self_powered\": {},\n", d.is_self_powered()));
    out.push_str(&format!("{pad2}    \"bus_powered\": {},\n", d.is_bus_powered()));
    out.push_str(&format!(
        "{pad2}    \"remote_wakeup\": {}\n",
        d.config_attributes.is_some_and(|a| a & 0x20 != 0)
    ));
    out.push_str(&format!("{pad2}  }}\n"));
    out.push_str(&format!("{pad2}],\n"));
    out.push_str(&format!("{pad2}\"interfaces\": ["));
    for (i, iface) in d.interfaces.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("\n{pad2}  {{\n"));
        out.push_str(&format!("{pad2}    \"number\": {},\n", iface.number));
        out.push_str(&format!("{pad2}    \"alternate\": {},\n", iface.alt));
        out.push_str(&format!("{pad2}    \"class_name\": \"{}\",\n", usb::class_name(iface.class)));
        out.push_str(&format!("{pad2}    \"class_code\": \"{:02x}:{:02x}:{:02x}\",\n", iface.class, iface.subclass, iface.protocol));
        opt_field(out, indent + 6, "name", iface.name.as_deref(), true);
        out.push_str(&format!("{pad2}    \"endpoints\": ["));
        for (j, ep) in iface.endpoints.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "\n{pad2}      {{ \"address\": \"{:#04x}\", \"direction\": \"{}\", \"transfer\": \"{}\", \"max_packet\": {}, \"interval\": {} }}",
                ep.address,
                if ep.input { "IN" } else { "OUT" },
                usb::transfer_type_name(ep.transfer),
                ep.max_packet,
                ep.interval
            ));
        }
        if !iface.endpoints.is_empty() {
            out.push('\n');
            out.push_str(&format!("{pad2}    "));
        }
        out.push_str("]\n");
        out.push_str(&format!("{pad2}  }}"));
    }
    if !d.interfaces.is_empty() {
        out.push('\n');
        out.push_str(&pad2);
    }
    out.push_str("]\n");
    out.push_str(&format!("{pad}}}"));
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn field(out: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    out.push_str(&format!(
        "{}\"{}\": \"{}\"{}\n",
        " ".repeat(indent),
        key,
        json_escape(value),
        if comma { "," } else { "" }
    ));
}

fn opt_field(out: &mut String, indent: usize, key: &str, value: Option<&str>, comma: bool) {
    match value {
        Some(v) => field(out, indent, key, v, comma),
        None => out.push_str(&format!("{}\"{}\": null{}\n", " ".repeat(indent), key, if comma { "," } else { "" })),
    }
}

fn opt_number_field(out: &mut String, indent: usize, key: &str, value: Option<u64>, comma: bool) {
    match value {
        Some(v) => out.push_str(&format!("{}\"{}\": {}{}\n", " ".repeat(indent), key, v, if comma { "," } else { "" })),
        None => out.push_str(&format!("{}\"{}\": null{}\n", " ".repeat(indent), key, if comma { "," } else { "" })),
    }
}

fn array_field(out: &mut String, indent: usize, key: &str, values: &[String], comma: bool) {
    out.push_str(&format!("{}\"{}\": [", " ".repeat(indent), key));
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("\"{}\"", json_escape(value)));
    }
    out.push_str(&format!("]{}\n", if comma { "," } else { "" }));
}

fn vid_pid(d: &Device) -> String {
    format!("{:04x}:{:04x}", d.vid, d.pid)
}

fn json_escape(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn escape_md(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn matching_bracket(input: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, c) in input[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
        } else if c == open {
            depth += 1;
        } else if c == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(start + offset);
            }
        }
    }
    None
}

fn json_field(object: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let pos = object.find(&needle)?;
    let colon = object[pos + needle.len()..].find(':')? + pos + needle.len();
    let rest = object[colon + 1..].trim_start();
    let value = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            match c {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_own_snapshot_devices() {
        let devices = usb::demo_scan(0);
        let json = json(&devices);
        let parsed = parse_snapshot_devices(&json).unwrap();
        assert!(parsed.iter().any(|d| d.name == "1-2" && d.vid == "07fd"));
    }

    #[test]
    fn diff_reports_added_removed_changed() {
        let before = parse_snapshot_devices(r#"{"devices":[{"name":"1-1","vid":"046d","pid":"c52b","label":"Old","class_name":"HID","speed_mbps":"12"}]}"#).unwrap();
        let mut after = usb::demo_scan(0);
        after.retain(|d| d.name == "1-1");
        let out = diff(&before, &after);
        assert!(out.contains("changed: 1"));
        assert!(out.contains("label: Old ->"));
    }
}
