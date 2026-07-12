---
id: module-metrics
type: module
anchors: [src/metrics.rs]
---
# metrics — per-device activity rates

Three backends: urbnum sysfs (unprivileged, linux, URB count not bytes), usbmon
(root, real bytes/s), demo (synthetic). Picks best available at runtime. Rate =
delta between ticks; main polls it each 1s rescan.
