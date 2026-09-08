# PenguKit — Implementation Plan (2026-09-08)

**Goal:** a from-scratch, Rust-native, Windows-independent CP2077 modding
toolkit ("WolvenKit for Linux"). Command name: `pengu`.

## Non-negotiables
- Zero Windows tooling/Wine/DLLs. Every file type served by Linux-native code.
- Portable across mainstream Linux distros: glibc x86_64 release binary +
  bundled `natives/`. No distro packages needed at runtime.
- Only optional external dep: `ffmpeg`, for `.bk2` preview only (auto-detected).
- Every release regression-tested against a local golden corpus (hashes of real
  game archives, generated once per game version).
- GPL-3.0. `NOTICE` credits WolvenKit/CP77Tools, RAD, ww2ogg/wwise-audio-tools.

## Ground truth (this machine)
- Game: `/var/home/Gigatone/Games/Cyberpunk 2077/` (Heroic). Archives in
  `archive/pc/{content,ep1,mod}`. `.archive` magic observed: `RDAR`, version `0x0c`.
- Reference natives on disk (Linux x86_64): `libkraken.so` (712KB),
  `libwwtools.so` (1.2MB) — copied into `natives/`.
- Mod library at `/var/home/Gigatone/Games/Mod Library/` = practical reference for
  real mod layouts (archive/pc/mod + .xl, r6/scripts+tweaks+config,
  red4ext/plugins/*, bin/x64/plugins/cyber_engine_tweaks/mods).
- ffmpeg ships `binkvideo` decoder (preview path for Phase 7).

## File-type coverage
- Lossless extract/repack for every RED4 type.
- Full typed tooling: `.xbm` (BC1-7, sRGB, cubemaps, mipmaps), `.mesh`<->glTF/glb,
  `.tweak`<->`.bin`, `.wem`<->`.ogg`, `.bk2` (extract/preview, then custom Rust
  Bink-1 encoder), text types (`.reds` `.lua` `.json` `.xl`) first-class.

## Command surface
`version` `hash` `self-check` (done) · `unbundle` `pack` `archive` (P1) ·
`info` (P2) · `uncook` `import` `export` (P3/P4) · `tweak` (P5) · `wwise` (P6) ·
video (P7) · `settings` + packaging (P8) · optional egui GUI (P9).

## Phases
- **P0 (current)** workspace, CLI scaffold, RED4 hashing, natives, LICENSE/NOTICE/README, corpus bootstrap
- **P1** RDAR reader (`unbundle`) + writer (`pack`) via libkraken.so; byte round-trip vs real archives; lock in hash behavior from the file table
- **P2** CR2W container read/write + `info`
- **P3** texture decode then encode (XBM<->DDS; BCn via vendored bc7enc/`cc` or pure-Rust codec)
- **P4** mesh -> glTF export, then import
- **P5** TweakDB: `.tweak` YAML <-> `.bin` (flats/groups)
- **P6** audio: WEM <-> OGG via libwwtools.so
- **P7** video: bk2 extract/preview (ffmpeg), then experimental Rust Bink-1 encoder
- **P8** distribution: tar.gz + AppImage + Flatpak + Homebrew + `scripts/build-release.sh`; `settings`
- **P9** optional egui texture/mesh viewer

## Verification strategy
- Golden corpus: `tests/corpus/golden.json` = {archive, path, sha256} extracted
  once per game version; regression gate.
- Round-trips: unbundle->pack->unbundle byte-identical; XBM->PNG->XBM;
  WEM->OGG playable; TweakDB round-trip.
- `pengu self-check` offline verification of natives + builtins.