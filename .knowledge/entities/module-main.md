---
id: module-main
type: module
anchors: [src/main.rs]
---
# main — App, event loop, all drawing, theme

Single owner of the ratatui render + crossterm event loop. 1s rescan tick drives
hot-plug diff. Theme = charm pastels, hand-aligned block — do NOT `cargo fmt` it.
All drawing lives here (no view module); detail pane switches USB vs PCI. Biggest
file — most churn lands here.

## Relations
- depends_on: module-usb
- depends_on: module-metrics
- depends_on: module-pci
