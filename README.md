# PenguKit

**WolvenKit for Linux** — a from-scratch, Rust-native, Windows-independent
modding toolkit for Cyberpunk 2077 (REDengine 4).

- Zero Windows: no Wine, no `.dll`, no `.exe`, no mixed-mode C++/CLI.
- Portable: single glibc x86_64 binary + bundled natives — see
  "Requirements & Distro Compatibility" below for exactly which distros it
  runs on (and which it does not).
- Distributed as a zip (see "Build"); AppImage/Flatpak/Homebrew packaging is
  planned.

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

## Requirements & Distro Compatibility

PenguKit ships **one build**: `PenguKit-<version>-linux-x86_64.zip` (glibc,
x86-64). It does **not** run on:

- 32-bit (i686) or non-x86-64 CPUs (ARM/aarch64, RISC-V, …) — no build exists.
- musl systems (Alpine, Adelie, …) — the binary and bundled natives are
  glibc-linked and cannot dlopen on musl.

Minimum glibc versions, measured from the shipped binaries:

| Component | Requires |
|---|---|
| `pengu` CLI | glibc ≥ 2.34 |
| bundled `libwwtools.so` | glibc ≥ 2.38 (any command that loads it) |

So, in practice on x86-64:

| Status | Distros |
|---|---|
| ✔ **Works** | Any distro on glibc ≥ 2.38: Arch / Manjaro, Fedora 39+, Ubuntu 23.10+ (incl. **24.04 LTS**), Debian 13+, openSUSE Tumbleweed and current rolling releases. |
| ⚠ **Partial** — CLI runs, but anything using `libwwtools.so` fails | glibc 2.34–2.37: Ubuntu 22.04 LTS / 23.04, Debian 12, Fedora 34–38, RHEL 9 / CentOS Stream 9. |
| ✘ **Does not work** | glibc < 2.34: Ubuntu ≤ 21.10, Debian ≤ 11, openSUSE Leap 15.x, RHEL ≤ 8 / CentOS 7–8. Also musl (Alpine) and any non-x86-64 CPU, regardless of glibc. |

The authoritative check is one command on the target machine:

```sh
ldd --version
```

The **GUI** (`pengukit`) additionally needs `python3-gobject` (PyGObject),
**GTK4** and **libadwaita**. The headless `pengu` CLI does not need any of these
and works on any machine that satisfies the table above.

- Fedora / Bazzite / Arch: usually already installed on GTK desktops.
- Debian / Ubuntu: `sudo apt install python3-gi gir1.2-gtk-4.0 gir1.2-adw-1`

## GUI

`pengukit` opens a WolvenKit-style desktop app (dark theme, left navigation
rail, Home dashboard of action tiles, live console pane):

- **Extract** — unpack a `.archive` into a folder, or list its contents.
- **Pack** — build a `.archive` from a folder (raw segments, game-valid).
- **Hash** — RED4 FNV-1a path hash + index CRC-64/XZ.
- **Self-Check** — verify the bundled native libraries.

It wraps the headless `pengu` CLI, so `pengukit <args>` still runs CLI
commands (e.g. `pengukit hash base\characters\head_average.mesh`).

GUI requirements are listed under "Requirements & Distro Compatibility".

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