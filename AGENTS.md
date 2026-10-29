# ShotCraft Engine

This repository contains one Rust library and one CLI for full-screen scrolling capture.
Keep capture, stitching, image encoding and cooperative stopping in the existing modules under `src/`.
The public capture interface is `shotcraft_engine::scrolling`; the binary is `scroll_capture`.

- Return `Error`/`Result` for runtime failures. Non-test code must not use unwrap, expect, panic, todo, unreachable or unimplemented.
- Keep `unsafe_code = "forbid"` and use safe dependency APIs for platform operations.
- Preserve the existing stitcher and stop semantics: after a committed frame, a stop returns the partial image.
- The library creates no windows and writes no capture files; file output belongs to the CLI or caller.
- Keep this a capture component. UI, OCR, tray, editing, upload and application settings are outside its scope.
- `backups/` is ignored local recovery material, not a workspace member or distribution input.
- Keep GitHub build triggers manual (`workflow_dispatch`) only. Update README when the library or CLI contract changes.
- Keep upstream copyright and license notices. Independently implement algorithm ideas; do not copy another product's code or assets.

Before committing, run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Prefer retained tests; add only tests needed for an accepted behavior change.
After dependency changes regenerate, never hand-edit, the third-party license file with cargo-about 0.9.2:

```sh
cargo about generate --workspace --locked about.hbs -o THIRD-PARTY-LICENSES.md
```

Platform build requirements, current backend limitations and invocation examples live in README.md.
