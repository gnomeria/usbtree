# Contributing

Thanks for taking the time to improve usbtree.

## Development

Install a stable Rust toolchain, then use the project tasks or Cargo directly:

```sh
cargo test
cargo clippy -- -D warnings
cargo build --release --locked
```

The fake device tree is useful for development without USB hardware:

```sh
cargo run -- --demo
cargo run -- --demo --dump
```

If you have [Task](https://taskfile.dev) installed, `task -l` lists the same common commands.

## Pull requests

- Keep changes focused and match the surrounding style.
- Add or update tests when changing parsing, tree behavior, device classification, or release/install logic.
- Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` before opening a pull request.
- Use Conventional Commit-style commit messages (`feat:`, `fix:`, `docs:`, `test:`, `chore:`). Release automation uses them to build the changelog.

## Screenshots

Screenshots are generated from `tapes/` with [VHS](https://github.com/charmbracelet/vhs). If UI changes affect the demo, run:

```sh
scripts/shots.sh
```

That command requires `vhs`, `ttyd`, and `ffmpeg`.
