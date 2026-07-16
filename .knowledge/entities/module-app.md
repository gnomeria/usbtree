---
id: module-app
type: module
anchors: [src/app.rs]
---
# app — App state and logic

Manages the core application state, including the device tree, selections, and updating USB/PCI metrics.

## Relations
- depends_on: module-usb
- depends_on: module-pci
- depends_on: module-metrics
