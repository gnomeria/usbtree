use crate::usb;
use std::collections::HashSet;

/// Non-TUI mode: print the tree once and exit.
pub fn dump(demo: bool) {
    let devices = if demo { usb::demo_scan(0) } else { usb::scan() };
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
