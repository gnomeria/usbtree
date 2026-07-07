# Walkthrough of `usbtree --demo` for the README/website.
# Regenerate with scripts/shots.sh (needs vhs) or the Screenshots workflow.
# Timeline of the demo loop: SSD plugs at 6 s / unplugs at 24 s,
# webcam unplugs at 14 s / returns at 20 s.

Output docs/screenshots/demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1300
Set Height 800
Set Padding 16
Set Theme "Catppuccin Mocha"

Hide
Type "./target/release/usbtree --demo"
Enter
Sleep 1.5s
Show

# initial tree with live sparklines
Sleep 3s

# walk down to the hub, fold and unfold it
Type "j"
Sleep 500ms
Type "j"
Sleep 500ms
Type "j"
Sleep 1s
Type "h"
Sleep 1.5s
Type "l"
Sleep 1.5s

# SSD hot-plug lands around here (t=6s) — let it flash green
Sleep 3s
Screenshot docs/screenshots/demo.png

# webcam unplug (t=14s): red crossed-out ghost + event log entry
Sleep 8s

# webcam returns (t=20s)
Sleep 5s

Hide
Type "q"
