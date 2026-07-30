use crate::usb;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .and_then(|pair| (!pair[1].starts_with("--")).then_some(pair[1].as_str()))
}

fn devices(demo: bool) -> Vec<usb::Device> {
    if demo { usb::demo_scan(0) } else { usb::scan() }
}

/// Non-TUI mode: print the tree once and exit.
pub fn dump(demo: bool) {
    let devices = devices(demo);
    let rows = usb::flatten(&devices, &HashSet::new());
    let rails = crate::ui::rails(&rows);
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

pub fn json(demo: bool) {
    print!("{}", crate::report::json(&devices(demo)));
}

pub fn markdown(demo: bool) {
    print!("{}", crate::report::markdown(&devices(demo)));
}

pub fn snapshot(demo: bool, path: &str) -> std::io::Result<()> {
    let body = crate::report::json(&devices(demo));
    fs::write(path, body)?;
    println!("snapshot saved to {}", Path::new(path).display());
    Ok(())
}

pub fn diff(demo: bool, path: &str) -> std::io::Result<()> {
    let previous = fs::read_to_string(path)?;
    let previous = crate::report::parse_snapshot_devices(&previous)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    print!("{}", crate::report::diff(&previous, &devices(demo)));
    Ok(())
}
