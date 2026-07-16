# Walkthrough of `usbtree --demo` for the README/website.
# Regenerate with scripts/shots.sh (needs vhs) or the Screenshots workflow.
# Timeline of the demo loop: SSD plugs at 3 s / unplugs at 12 s,
# webcam unplugs at 7 s / returns at 10 s. The eject-dialog shot and the
# filter must land while the SSD is present (t < 12).

Output docs/screenshots/demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1500
Set Height 800
Set Padding 16
Set Margin 20
Set WindowBar Colorful
Set Theme "Catppuccin Mocha"

Hide
Type "./target/release/usbtree --demo"
Enter
Sleep 1.5s
Show

# initial tree with live sparklines (SSD hot-plug lands ~t=3, flashing green)
Sleep 1.5s

# walk to the hub, fold to a +N badge, unfold again
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 500ms
Type "h"
Sleep 800ms
Type "l"
Sleep 800ms

# drop onto the SanDisk SSD, open the safe-eject dialog, confirm with `e`.
# in --demo the SSD is dropped from the scan, so it plays a real unplug:
# crossed-out ghost + event-log entry + "ejected" toast.
Type "j"
Sleep 250ms
Type "j"
Sleep 250ms
Type "j"
Sleep 250ms
Type "j"
Sleep 500ms
Type "e"
Sleep 700ms
Screenshot docs/screenshots/demo.png
Type "e"
Sleep 2s

# live filter: `/` opens the search box; "keychron" narrows to the nested
# keyboard, revealing its collapsed parent hub. Esc clears.
Type "/"
Sleep 600ms
Type "keychron"
Sleep 2.5s
Escape
Sleep 1s

# theme picker: `t` opens the modal, `j` scrolls down to live-preview themes
Type "t"
Sleep 1s
Type "j"
Sleep 700ms
Type "j"
Sleep 700ms
Type "j"
Sleep 700ms
Type "j"
Sleep 1.5s
Enter
Sleep 500ms

Hide
Type "q"
