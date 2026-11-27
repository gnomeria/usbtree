---
id: module-app
type: module
anchors: [src/app.rs, src/usb.rs]
---
# app — App state and logic

Manages the core application state, including the device tree, selections, and updating USB/PCI metrics.

Copying full USB details with `Y` includes each storage path and its mount points in scan order.

## Relations
- depends_on: module-usb
- depends_on: module-pci
- depends_on: module-metrics
