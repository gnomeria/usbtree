---
id: module-usb
type: module
anchors: [src/usb.rs, src/usb/storage.rs, src/usb/storage_macos.rs, src/usb/storage_windows.rs]
---
# usb — nusb scan, tree build, usb.ids, demo

Scans via nusb (no libusb, no root). Builds sysfs-style names (`1-1.4`), flattens
+ folds the tree, diffs for hot-plug. Parses bundled usb.ids + user overrides at
`~/.config/usbtree/overrides.ids`. Descriptor read is cfg-gated per-OS (linux
sysfs / nusb / stub). `demo_scan(t)` synthesizes a fake tree for `--demo`.
Linux scans also join block devices to their nearest USB interface in sysfs and
follow holders before reading mount points from the process mount namespace.
macOS maps IOMedia ancestors to nusb registry entry IDs. Windows maps disk
devnodes to nusb instance IDs, then joins volumes by storage device number.

## Relations
- decided_by: decision-nusb-no-libusb
- gotcha: gotcha-demo-tape-coupling
