---
id: module-usb
type: module
anchors: [src/usb.rs]
---
# usb — nusb scan, tree build, usb.ids, demo

Scans via nusb (no libusb, no root). Builds sysfs-style names (`1-1.4`), flattens
+ folds the tree, diffs for hot-plug. Parses bundled usb.ids + user overrides at
`~/.config/usbtree/overrides.ids`. Descriptor read is cfg-gated per-OS (linux
sysfs / nusb / stub). `demo_scan(t)` synthesizes a fake tree for `--demo`.

## Relations
- decided_by: decision-nusb-no-libusb
- gotcha: gotcha-demo-tape-coupling
