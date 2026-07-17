# Changelog

## [0.1.1](https://github.com/gnomeria/usbtree/compare/v0.1.0...v0.1.1) (2026-07-17)


### Documentation

* add Homebrew install and align install section UI ([#71](https://github.com/gnomeria/usbtree/issues/71)) ([71c1ce7](https://github.com/gnomeria/usbtree/commit/71c1ce789f3cc0ed61e317b104b9e6cfbf0a644d))
* point Homebrew install link at gnomeria/tap ([#73](https://github.com/gnomeria/usbtree/issues/73)) ([171c884](https://github.com/gnomeria/usbtree/commit/171c884c0d192391c1e6b7c24279b44b3e9df0de))

## [0.1.0](https://github.com/gnomeria/usbtree/compare/v0.0.10...v0.1.0) (2026-07-16)


### Bug Fixes

* atomic usb.ids cache update and Windows release check ([#68](https://github.com/gnomeria/usbtree/issues/68)) ([aeac78a](https://github.com/gnomeria/usbtree/commit/aeac78ac9b0693efee6c1a0acd392e3dbb159b38))

## [0.0.10](https://github.com/gnomeria/usbtree/compare/v0.0.9...v0.0.10) (2026-07-16)


### Features

* **ui:** set tokyo night as the default theme ([#66](https://github.com/gnomeria/usbtree/issues/66)) ([b656ecc](https://github.com/gnomeria/usbtree/commit/b656ecc97939ec6cf56cd845d513d008c6096535))

## [0.0.9](https://github.com/gnomeria/usbtree/compare/v0.0.8...v0.0.9) (2026-07-16)


### Features

* add --nerd-font and --ascii icon fallbacks ([#64](https://github.com/gnomeria/usbtree/issues/64)) ([b481119](https://github.com/gnomeria/usbtree/commit/b48111916730e8300094f110dbd521ba6357fba9))
* add interactive theme picker and expand color themes ([#65](https://github.com/gnomeria/usbtree/issues/65)) ([6cc6cd1](https://github.com/gnomeria/usbtree/commit/6cc6cd1af8072672b80af6fcc79ffb1cc95586b5)), closes [#58](https://github.com/gnomeria/usbtree/issues/58)


### Bug Fixes

* **tui:** strip VS16 from icons to stop redraw ghosting ([#55](https://github.com/gnomeria/usbtree/issues/55)) ([9a316d8](https://github.com/gnomeria/usbtree/commit/9a316d8010c37f3cfa78145bfeab997fd892e712))


### Refactors

* modularize main.rs and extract test files ([#63](https://github.com/gnomeria/usbtree/issues/63)) ([c0af1e6](https://github.com/gnomeria/usbtree/commit/c0af1e666b2f0b41471266ee28e121f31625bd14))


### Documentation

* **skills:** rewrite skills for rust, add ci + knowledge-graph, seed .knowledge ([#56](https://github.com/gnomeria/usbtree/issues/56)) ([f259588](https://github.com/gnomeria/usbtree/commit/f259588e40d80eef2dc3f27831b77439ae510f52))
* trim README and site, note binary sizes ([#53](https://github.com/gnomeria/usbtree/issues/53)) ([f3fccf7](https://github.com/gnomeria/usbtree/commit/f3fccf799cde8e434972037ca848e589a8645cc7))

## [0.0.8](https://github.com/gnomeria/usbtree/compare/v0.0.7...v0.0.8) (2026-07-10)


### Features

* **pci:** add prog-if, subsystem, link, numa, iommu, power to detail pane ([#48](https://github.com/gnomeria/usbtree/issues/48)) ([cb4b235](https://github.com/gnomeria/usbtree/commit/cb4b23597183bad2f858be04b37468930e4849ec))
* **usb:** safe eject storage devices with confirm dialog ([#51](https://github.com/gnomeria/usbtree/issues/51)) ([c0bb57b](https://github.com/gnomeria/usbtree/commit/c0bb57b404790c1b654998f253093df29c516606))


### Bug Fixes

* **tui:** filter finds devices inside collapsed hubs ([#50](https://github.com/gnomeria/usbtree/issues/50)) ([3485941](https://github.com/gnomeria/usbtree/commit/3485941964336fe933ce47b64dd201ca0ddea100))

## [0.0.7](https://github.com/gnomeria/usbtree/compare/v0.0.6...v0.0.7) (2026-07-09)


### Features

* **usb:** expose device subclass/protocol + bus controller type ([#44](https://github.com/gnomeria/usbtree/issues/44)) ([4e39c3b](https://github.com/gnomeria/usbtree/commit/4e39c3b67f9715230f3e4a1763739e8a303dad4b))


### Bug Fixes

* **usb:** clean bus names on Windows ([#43](https://github.com/gnomeria/usbtree/issues/43)) ([c569cb0](https://github.com/gnomeria/usbtree/commit/c569cb0919c6bd8bb64075f16edcc30146cc3603))


### Documentation

* **readme:** merge duplicate installer env-var tables ([#42](https://github.com/gnomeria/usbtree/issues/42)) ([6204f4b](https://github.com/gnomeria/usbtree/commit/6204f4b16af5233e572a9b01f4e09ca8a62d0230))
* refresh demo screenshots ([#39](https://github.com/gnomeria/usbtree/issues/39)) ([cdd1a6f](https://github.com/gnomeria/usbtree/commit/cdd1a6fe06f94069482110062cb8157fd7452adf))
* refresh screenshots ([#45](https://github.com/gnomeria/usbtree/issues/45)) ([a39b494](https://github.com/gnomeria/usbtree/commit/a39b494dca0fbf94e9db1434600b4af9894a2674))
* **site:** add OS toggle + auto-detect to hero install ([#41](https://github.com/gnomeria/usbtree/issues/41)) ([3e21d01](https://github.com/gnomeria/usbtree/commit/3e21d014aa3d0607c6c3b5f666fd7774bf3fd47a))

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
