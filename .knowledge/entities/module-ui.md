---
id: module-ui
type: module
anchors: [src/ui.rs, src/usb.rs]
---
# ui — Drawing and Theme

Handles all `ratatui` rendering logic and UI themes (Catppuccin/Charm pastels). This is the biggest file in the project. Do NOT `cargo fmt` the hand-aligned theme blocks.

The USB detail pane shows storage nodes in scan order, with mount points below each node.

## Relations
- depends_on: module-app
- depends_on: module-usb
