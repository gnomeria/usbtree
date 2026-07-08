# Changelog

## [0.0.6](https://github.com/gnomeria/usbtree/compare/v0.0.5...v0.0.6) (2026-07-08)


### Features

* **ui:** show interfaces + endpoints in detail panel ([#37](https://github.com/gnomeria/usbtree/issues/37)) ([edcb157](https://github.com/gnomeria/usbtree/commit/edcb1577e129ac1cbe281d2b992152419e8259b5))


### Bug Fixes

* **install:** brace  to survive bash 3.2 unbound-var ([#35](https://github.com/gnomeria/usbtree/issues/35)) ([6117e75](https://github.com/gnomeria/usbtree/commit/6117e75c82b3d58e5226a56d6df24fa7113ee779))
* **updatelist:** use vcrhonek/hwdata URL so --updatelist works ([#38](https://github.com/gnomeria/usbtree/issues/38)) ([2b4d5cf](https://github.com/gnomeria/usbtree/commit/2b4d5cfde834ffb2b2869dd458750fa7bcc477ac))

## [0.0.5](https://github.com/gnomeria/usbtree/compare/v0.0.4...v0.0.5) (2026-07-08)


### Features

* **install:** add PowerShell installer for Windows ([bbfcb87](https://github.com/gnomeria/usbtree/commit/bbfcb8758084827f7f5ca75e718e7a00669dfd5b))


### Bug Fixes

* **build:** statically link CRT so Windows exe runs without VC++ Redistributable ([#33](https://github.com/gnomeria/usbtree/issues/33)) ([bbfcb87](https://github.com/gnomeria/usbtree/commit/bbfcb8758084827f7f5ca75e718e7a00669dfd5b))

## [0.0.4](https://github.com/gnomeria/usbtree/compare/v0.0.3...v0.0.4) (2026-07-08)


### Features

* **ui:** distinct colors per USB speed tier ([#31](https://github.com/gnomeria/usbtree/issues/31)) ([7f7d123](https://github.com/gnomeria/usbtree/commit/7f7d12321e12a5cb8297c79d90cfd58fdd3104eb))

## [0.0.3](https://github.com/gnomeria/usbtree/compare/v0.0.2...v0.0.3) (2026-07-08)


### Features

* **install:** symlink prompt for sudo usbtree, fix PATH trap docs ([#25](https://github.com/gnomeria/usbtree/issues/25)) ([ccd1b5d](https://github.com/gnomeria/usbtree/commit/ccd1b5d6816a52767147b1036e72b927fa3c3da9))
* **tui:** name modprobe usbmon in header + docs for bytes/s ([#22](https://github.com/gnomeria/usbtree/issues/22)) ([f0991a7](https://github.com/gnomeria/usbtree/commit/f0991a79265717e2cba2f0ca6796869aaca344c3))
* **tui:** show device max power (bMaxPower) in detail pane ([#20](https://github.com/gnomeria/usbtree/issues/20)) ([9fcfe3a](https://github.com/gnomeria/usbtree/commit/9fcfe3aed566f95f8bee2375e58497c3641fae35))
* **usb:** read bMaxPower on macOS via config descriptor ([#26](https://github.com/gnomeria/usbtree/issues/26)) ([5717249](https://github.com/gnomeria/usbtree/commit/5717249d7f095af76fe7617f847f64a71a41abbf))


### Documentation

* add cross-platform feature matrix to readme and site ([#23](https://github.com/gnomeria/usbtree/issues/23)) ([81c6ad7](https://github.com/gnomeria/usbtree/commit/81c6ad71e5d4fd51ae9dcbb940fdf74d1ae09a06))
* refresh demo screenshots ([#27](https://github.com/gnomeria/usbtree/issues/27)) ([f744923](https://github.com/gnomeria/usbtree/commit/f744923de549beb566e41c757a2cee692e59f847))

## [0.0.2](https://github.com/gnomeria/usbtree/compare/v0.0.1...v0.0.2) (2026-07-08)


### Features

* **metrics:** mark per-device activity unavailable on macOS/Windows ([#11](https://github.com/gnomeria/usbtree/issues/11)) ([b8fdd06](https://github.com/gnomeria/usbtree/commit/b8fdd0619ef97454b38e0bab8a4ef33fb44b8222))
* **tui:** live tree filter/search with ([#19](https://github.com/gnomeria/usbtree/issues/19)) ([c46152b](https://github.com/gnomeria/usbtree/commit/c46152b180fdb1ba83a5688bc785bdb97bed4895))
* **tui:** mouse support — click select, scroll, right-click copy menu ([#18](https://github.com/gnomeria/usbtree/issues/18)) ([69552db](https://github.com/gnomeria/usbtree/commit/69552db34b17b7a7c57eb746d136154d226cef4e))
* **tui:** pane focus + responsive metric columns ([#15](https://github.com/gnomeria/usbtree/issues/15)) ([2595b39](https://github.com/gnomeria/usbtree/commit/2595b3944c5fa277b6486d59ce7734dcc1a248e4))
* **tui:** right-align metrics in fixed columns, keep ghost data red ([#13](https://github.com/gnomeria/usbtree/issues/13)) ([a2cae7e](https://github.com/gnomeria/usbtree/commit/a2cae7ef08eb50fde7edf22029b2a057237b746e))
* **tui:** show app version + new-release notice bottom-right ([#17](https://github.com/gnomeria/usbtree/issues/17)) ([b7c31ba](https://github.com/gnomeria/usbtree/commit/b7c31bad3429bcf0b6f2eb531941475bd44ee78d))
* **tui:** yank device id/details to clipboard ([#14](https://github.com/gnomeria/usbtree/issues/14)) ([8fdab00](https://github.com/gnomeria/usbtree/commit/8fdab0099aa4a07532aa08e672d7b7459dd671ce))


### Bug Fixes

* **release:** build linux binaries as static musl ([#16](https://github.com/gnomeria/usbtree/issues/16)) ([2cb20e2](https://github.com/gnomeria/usbtree/commit/2cb20e2817dbc7e39a4ea8792d0fd9fb9a375328))

## 0.0.1 (2026-07-07)


### Chores

* bootstrap release-please at 0.0.1 ([2a78ebf](https://github.com/gnomeria/usbtree/commit/2a78ebfdd41262d98889e29a6672463440a47dfd))

## Changelog

All notable changes to this project will be documented in this file.

This project uses [release-please](https://github.com/googleapis/release-please), which updates this changelog from Conventional Commit messages when preparing a release.
