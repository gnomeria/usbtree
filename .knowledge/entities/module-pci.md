---
id: module-pci
type: module
anchors: [src/pci.rs]
---
# pci — PCI device detail for the detail pane

Reads sysfs PCI attrs (prog-if, subsystem, link speed/width, numa, iommu group,
power state) for the host controller behind a USB bus. Feeds main's detail pane
when a PCI root is selected. Linux-only data; degrades to empty elsewhere.
