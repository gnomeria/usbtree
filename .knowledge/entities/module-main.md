---
id: module-main
type: module
anchors: [src/main.rs]
---
# main — Entry point

Simple entry point. Sets up terminal, handles CLI arguments (via `cli.rs`), and delegates to `App` in `app.rs`.

Note: This file was previously the monolith owner of all logic. `App` state is now in `app.rs`, event loop in `events.rs`, and drawing in `ui.rs`.

## Relations
- depends_on: module-app
- depends_on: module-events
- depends_on: module-ui
- depends_on: module-cli
