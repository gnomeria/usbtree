---
id: gotcha-demo-tape-coupling
type: gotcha
anchors: [src/usb.rs, tapes/demo.tape]
---
# demo_scan timings are load-bearing for the VHS tape

`demo_scan(t)` runs a fixed 30s loop: SSD in @6s / out @24s, webcam out @14s /
back @20s. `tapes/demo.tape` sleeps are timed to catch those exact events on
screen. Change the loop timings or which devices appear → the tape records a
walkthrough that misses its plug/unplug moments. Re-check the tape after any
demo_scan edit.

## Relations
- part_of: module-usb
