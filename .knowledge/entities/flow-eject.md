---
id: flow-eject
type: flow
anchors: [src/usb.rs, src/main.rs]
---
# eject — safe unplug, `e` key to power-off

`e` on a storage device → modal confirm dialog (main.rs) → `usb::eject()` on a
worker thread; result comes back over `eject_rx` so a slow power-off never
freezes the UI ("ejecting…" line meanwhile). Linux only: udisksctl unmount +
power-off per backing disk, unprivileged via polkit (`// ponytail:` marks the
mac/windows gap). Busy partition = that drive stays powered, others proceed.
Hub eject grabs all downstream disks. `--demo` fakes it via `demo_ejected` set.

## Relations
- depends_on: module-usb
- depends_on: module-main
