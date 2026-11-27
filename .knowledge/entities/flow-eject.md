---
id: flow-eject
type: flow
anchors: [src/app.rs, src/usb.rs, src/usb/storage.rs]
---
# eject — safe unplug, `e` key to power-off

`e` on a storage device → modal confirmation in `src/app.rs` → `usb::eject()` on a
worker thread; result comes back over `eject_rx` so a slow power-off never
freezes the UI ("ejecting…" line meanwhile). Linux only: udisksctl unmount +
power-off per backing disk, unprivileged via polkit. Busy partitions and active
block holders keep that drive powered. Eject resolves whole disks again using
the display's nearest-interface rule. `--demo` fakes it via `demo_ejected` set.

## Relations
- depends_on: module-usb
- depends_on: module-app
