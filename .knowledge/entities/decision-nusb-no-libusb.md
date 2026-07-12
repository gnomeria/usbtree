---
id: decision-nusb-no-libusb
type: decision
anchors: [src/usb.rs, Cargo.toml]
---
# Scan with nusb, not libusb; never require root

Pure-Rust nusb over libusb: no C toolchain, no system lib, cross-compiles clean
for the 3-OS release matrix. Unprivileged by design — enumeration + descriptors
work without root on all platforms. Cost: real byte/s traffic needs usbmon (root),
so unprivileged metrics fall back to URB counts. Rejected: libusb (build friction,
unsafe FFI), always-root (kills the "just run it" pitch).

## Relations
- part_of: module-usb
