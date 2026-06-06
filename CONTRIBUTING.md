# Contributing to BLACKOUT

Thanks for helping make privacy a one-tap thing for everyone.

## Ground rules (non-negotiable)

BLACKOUT's whole value is trust. Contributions must keep these true:

- **Local only.** No network calls, ever. No telemetry, analytics, accounts, ads.
- **Originals are sacred.** CLEAN only ever writes copies; it never modifies input.
- **Honesty over theater.** If an OS blocks an action, the app must *say so* — never
  fake success. A privacy tool that pretends is worse than useless.

## Where things live

```
crates/blackout-core/      pure-Rust CLEAN engine (no OS code) — keep it portable
crates/blackout-platform/  OPSEC/LOCKDOWN/PANIC, cfg-gated per OS (macos/ios/android/other)
crates/blackout-cli/       terminal front-end
app/                       universal Tauri app (shared web UI + thin command layer)
```

Adding support for a new OS = add one file in `blackout-platform`. The engine and
UI shouldn't change.

## Dev loop

```bash
cargo test                       # engine + platform unit tests
cd app && npm run tauri dev      # hot-reload desktop app
```

Please add a unit test when you touch a parser in `blackout-core`.

## Pull requests

- Keep the diff focused; explain *why* in the description.
- Run `cargo test` and `cargo build` before pushing.
- New metadata format? Add a test fixture + a stripping test.

Licensed under MIT OR Apache-2.0; by contributing you agree your work is too.
