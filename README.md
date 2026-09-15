# PenguKit

**WolvenKit for Linux** — a from-scratch, Rust-native, Windows-independent
modding toolkit for Cyberpunk 2077 (REDengine 4).

- Zero Windows: no Wine, no `.dll`, no `.exe`, no mixed-mode C++/CLI.
- Portable: single glibc x86_64 binary + bundled natives, runs unmodified on
  essentially every mainstream Linux distro.
- Distributed as tar.gz, AppImage, Flatpak and Homebrew.

## Status

Active development. Phase 1 (RDAR archive read/write) complete — see
`docs/PLAN.md`.

Implemented so far:

| Command | Status |
|---|---|
| `pengukit` (GUI) | works — WolvenKit-style GTK4/Libadwaita app |
| `pengu version` | works |
| `pengu hash`    | works (RED4 FNV-1a path hashing + index CRC-64/XZ) |
| `pengu self-check` | works (verifies bundled natives) |
| `unbundle` `pack` `archive` | Phase 1 (lossless read/write round-trip verified) |
| `info`           | Phase 2 |
| `uncook` `import` `export` | Phases 3/4 |
| `tweak`          | Phase 5 |
| `wwise`          | Phase 6 |
| video / bk2      | Phase 7 |
| `settings` + packaging | Phase 8 |

## GUI

`pengukit` opens a WolvenKit-style desktop app (dark theme, left navigation
rail, Home dashboard of action tiles, live console pane):

- **Extract** — unpack a `.archive` into a folder, or list its contents.
- **Pack** — build a `.archive` from a folder (raw segments, game-valid).
- **Hash** — RED4 FNV-1a path hash + index CRC-64/XZ.
- **Self-Check** — verify the bundled native libraries.

It wraps the headless `pengu` CLI, so `pengukit <args>` still runs CLI
commands (e.g. `pengukit hash base\characters\head_average.mesh`).

Requirements: `python3-gobject`, GTK4 and libadwaita (standard on Bazzite,
Fedora and Arch; any GTK4 desktop works).

```sh
./pengukit                # run the GUI from a checkout
python3 gui/pengu_gui.py --self-test   # headless plumbing check
```

Install a launcher command + icon + desktop entry (undo anytime with
`rm ~/.local/bin/pengukit ~/.local/share/applications/pengukit.desktop`):

```sh
ln -s "$PWD/pengukit" ~/.local/bin/pengukit
cp icons/pengukit.svg ~/.local/share/icons/hicolor/scalable/apps/pengukit.svg
```

## Layout

```
crates/pengu-core/     hashing, endian IO, errors, native discovery
crates/pengu-archive/  RDAR .archive read/write (Phase 1)
crates/pengu-red4/     CR2W container (Phase 2)
crates/pengu-texture/  XBM<->DDS, BCn codecs (Phase 3)
crates/pengu-mesh/     .mesh <-> glTF (Phase 4)
crates/pengu-tweakdb/  TweakDB YAML <-> .bin (Phase 5)
crates/pengu-audio/    WEM <-> OGG (Phase 6)
crates/pengu-video/    Bink .bk2 (Phase 7)
crates/pengu-cli/      the `pengu` binary
natives/               libkraken.so, libwwtools.so (bundled, not installed)
```

## Build

```sh
cargo build --release
# binary: target/release/pengu
```

## Install

The `pengu` binary locates bundled natives automatically: beside the
executable, in `$PENGU_NATIVE_DIR`, `~/.local/share/pengu/natives`, or
`/app/lib/pengu` (Flatpak). Keep `natives/` next to the binary for a
self-contained install.

## License

GPL-3.0 — see `LICENSE`. Attribution for reference projects and bundled
libraries in `NOTICE`.