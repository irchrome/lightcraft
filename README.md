# LightCraft IR

An infrared photography fork of [storytold/lightcraft](https://github.com/storytold/lightcraft) — the open-source, pure-Rust photo library and raw developer — maintained by [irchrome.art](https://irchrome.art).

**Website:** [https://irchrome.art](https://irchrome.art)  
**Upstream project:** [https://github.com/storytold/lightcraft](https://github.com/storytold/lightcraft)

LightCraft IR adds **native infrared (IR) raw processing** to LightCraft: sRGB-space channel swaps and IR mixes, soft gamut compression for saturated IR skies, camera-bound DCP profiles, and extended raw camera support, while staying rebase-friendly with upstream LightCraft.

## Download (macOS)

Prebuilt macOS (Apple Silicon, `aarch64`) binaries are available on the [Releases](https://github.com/irchrome/lightcraft/releases/latest) page:

1. Download `LightCraft-IR-*-macos-aarch64.dmg` from [GitHub Releases](https://github.com/irchrome/lightcraft/releases/latest).
2. Open the DMG and drag **`LightCraft IR.app`** into `/Applications`.
3. Because community fork builds are ad-hoc signed (not notarized by Apple), on first launch run:
   ```sh
   xattr -dr com.apple.quarantine "/Applications/LightCraft IR.app"
   ```
   (or allow it in **System Settings → Privacy & Security → Open Anyway**).

> **Note:** Prebuilt GitHub releases are pure Rust (`MIT OR Apache-2.0`) and do **not** link LibRaw. If you need Nikon High Efficiency NEF (`HE` / `HE★`) decoding, build locally with `--features lightcraft/nikon-he` against a local LibRaw build (see [`IR-FORK.md`](IR-FORK.md)).

## What the fork adds today

- **RGB Channel Mixer** with a neutral picker and red/blue swap, carried from upstream PR #3579 (taken before upstream review).
- **Infrared channel mixes in sRGB primaries** (`Swap Red and Blue (sRGB)` and `More IR mixes`: `Red ↔ Green`, `Green ↔ Blue`, `Rotate R → G → B`, `Rotate R → B → G`, `Red ↔ Blue 70%`, `Red ↔ Blue 30%`, `Mono from Red`). Mixes replace each other cleanly instead of stacking, clicking the active mix toggles back to the base mixer, and manual edits are preserved.
- **Soft gamut compression for infrared edits** (ACES Reference Gamut Compression on CPU and GPU, applied only to IR edits with a user DCP profile or non-neutral channel mixer). Prevents flat yellow/cyan sky fills when foliage white balance drives a channel deeply negative; ordinary photos render identically to upstream.
- **Camera-bound DCP profiles:** imported `.dcp` camera profiles are offered and accepted only for raw files from the matching camera model (as in Lightroom), and never on JPEGs. LUT profiles remain available for all photos.
- **Compressed 12-bit Olympus ORF decoding**, ported from upstream PR #360. Upstream withdrew its own compressed ORF decoder pending a provenance review and has an open objection to PR #360; this fork keeps the port and marks it as disputed. It is not part of upstream LightCraft.
- **Optional Nikon High Efficiency NEF (`HE` / `HE★`) decoding** via an opt-in local LibRaw shim (`crates/nikon-he`, feature `nikon-he`, off by default).
- **Fork identification** in the About window and separate macOS app bundle (`LightCraft IR.app`, `art.irchrome.lightcraft-ir`) so it installs cleanly alongside upstream LightCraft.

## Building from source (macOS)

Upstream's build and test instructions apply unchanged — see [`AGENTS.md`](AGENTS.md), [`docs/`](docs/) and upstream's [README](https://github.com/storytold/lightcraft/blob/main/README.md). The fork's own architecture and rules are documented in [`IR-FORK.md`](IR-FORK.md).

To build and install `/Applications/LightCraft IR.app` locally:

```sh
./tools/install-macos-app.sh --install
```

Add `--nikon-he` if you have a local LibRaw build in `~/Documents/_vendor/LibRaw` (or `LIBRAW_DIR`), or `--dmg dist/release/LightCraft-IR.dmg` to produce a distributable disk image.

## Branches

| Branch | What it is |
|---|---|
| `main` | an exact mirror of upstream `main`, fast-forward only |
| `ir` | upstream `main` plus the IR patch series, rebased on upstream (`tools/sync-upstream.sh`, log in `SYNC-LOG.md`). History of `ir` is rewritten on sync; a tag `ir-pre-sync-<date>` marks the state before each rebase |

## Compatibility

Develop settings written by this fork carry extra IR fields (`ir_mix`). Upstream LightCraft ignores unknown fields and **drops them silently when it saves**, so do not open the same library with both builds simultaneously.

## Licence and trademarks

`MIT OR Apache-2.0`, as upstream ([`LICENSE-MIT`](LICENSE-MIT), [`LICENSE-APACHE`](LICENSE-APACHE), [`NOTICE`](NOTICE)). The ArtCraft name, wordmark and logos are trademarks of the ArtCraft Team and are not part of this fork; `docs/brand/` was removed as upstream's licence requires. This fork is maintained independently at [irchrome.art](https://irchrome.art) and is not affiliated with or endorsed by ArtCraft.

No Adobe camera profile (`.dcp`), lens profile or third-party LUT is included in or derived from for this repository (see [`assets/ATTRIBUTION.md`](assets/ATTRIBUTION.md)).
