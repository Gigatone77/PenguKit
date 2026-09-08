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
  `archive/pc/{content,ep1,mod}`. `.archive` magic observed: `RDAR`, version `0x0c`
  (version 12). Verified byte layout against `basegame_1_engine.archive`
  (1677524992 B, nfiles=4115, nsegs=15220).
- Reference natives on disk (Linux x86_64): `libkraken.so` (712KB),
  `libwwtools.so` (1.2MB) — copied into `natives/`.
- Mod library at `/var/home/Gigatone/Games/Mod Library/` = practical reference for
  real mod layouts (archive/pc/mod + .xl, r6/scripts+tweaks+config,
  red4ext/plugins/*, bin/x64/plugins/cyber_engine_tweaks/mods).
- ffmpeg ships `binkvideo` decoder (preview path for Phase 7).

## RDAR v12 archive format (verified 2026-09-08, source: WolvenKit + real file)
On-disk (all little-endian):
- Header 40 B: magic u32 "RDAR"(0x52444152), version u32 (12),
  indexPosition u64, indexSize u32, debugPosition u64, debugSize u32,
  filesize u64. Then at offset 40 a u32 customDataLength (0 for game archives);
  optional LxrsFooter custom data starts at Header.EXTENDED_SIZE (0xAC).
  indexPosition is an ABSOLUTE file offset where the index starts.
- Index at `indexPosition`: tableOffset u32, tableSize u32, crc u64,
  nFileEntries u32, nFileSegments u32, nDependencies u32.
  Followed by nFileEntries `FileEntry` (56 B: hash u64, timestamp u64,
  numInlineBufferSegments u32, segmentsStart u32, segmentsEnd u32,
  ResourceDependenciesStart u32, ResourceDependenciesEnd u32, sha1[20]) —
  ALL entries first — then nFileSegments `FileSegment`
  (16 B: offset u64, zsize u32, size u32), then nDependencies u64.
  Index body size == 28 + 56*nEntries + 16*nSegments + 8*nDeps == indexSize.
  (indexSize = 16 + table body: u32 tableOffset(=8) + u32 tableSize(body+8) +
  u64 crc + body.)
- Index CRC = CRC-64/XZ (reflected poly 0xC96C5795D7870F42, init + xorout both
  all-ones) over the table body bytes only — VERIFIED on basegame
  (0x7654580bfe77583f) and matches WolvenKit `Crc64.Compute`.
- Each file = segments[segmentsStart..segmentsEnd]. Segment 0 is the cooked
  main file; following segments are buffers. If zsize == size -> stored raw
  (copy `size` bytes from `offset`); if compressed the block at `offset` is
  "KRAK"(LE) + u32 sourceSize + Kraken payload of zsize-8 bytes.
  `Kraken_Decompress(payload, zsize-8, dst, size)` returns size (verified ->
  CR2W magic out).
- Timestamp is a Windows FILETIME (100ns since 1601); keep as raw u64 for now.
- Entry hash = **FNV-1a 64** of the resource path — NOT CRC-64. VERIFIED
  2026-09-08: 3,022 of 4,115 basegame file-table hashes match FNV-1a (init
  0xCBF29CE484222325, prime 0x00000100000001B3) of the real internal paths
  from WolvenKit `archivehashes.csv` (the rest post-date that snapshot).
  Paths are sanitized per WolvenKit `ResourcePath.SanitizePath`: trim
  `' " / \ space \n \r`, collapse `/` `\` to a single `\`, lowercase.
  `pengu hash` prints fnv1a / red4_hash / crc64_xz.

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