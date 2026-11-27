# LightCraft IR

An infrared photography fork of [storytold/lightcraft](https://github.com/storytold/lightcraft) — the open-source, pure-Rust photo library and raw developer — maintained by [irchrome.art](https://irchrome.art).

**Website:** [https://irchrome.art](https://irchrome.art)  
**Upstream project:** [https://github.com/storytold/lightcraft](https://github.com/storytold/lightcraft)

LightCraft IR adds **native infrared (IR) raw processing** to LightCraft: dedicated IR workspaces (`Tools ▸ Infrared` and three IR sections inside `Edit`), 46 clean-room sRGB channel mixes, soft gamut compression for saturated IR skies, camera-bound DCP profiles with Lightroom camera recovery, and extended raw camera support, while staying rebase-friendly with upstream LightCraft.

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

## What LightCraft IR adds for infrared photography

- **Dedicated `Infrared` tab (`Tools ▸ [ Edit | Infrared ]`, `Shift+I`, or the sun icon on the right strip):** a streamlined single-column workflow strictly ordered for IR raw processing: `Camera Profile` (camera-bound DCP quick list with live hover preview + `Amount`) → `White Balance & Color` (`W` picker, `Temp`/`Tint`, `Exposure`, `Vibrance`, `Saturation`) → `Infrared Channel Mixer` (neutral picker, `Swap Red and Blue (sRGB)`, `More IR mixes`, `IR Mix Catalog`, and 3×3 sliders).
- **Three specialized IR sections directly inside `Edit`:**
  - **`IR-FALSE-COLOR`** — WB and neutral pickers, `Swap Red and Blue (sRGB)`, filterable false-color mix catalog (`Built-in`, `Swaps`, `Swap Variants`, `Creative`, `Fitted Looks`), plus `Vibrance`/`Saturation`.
  - **`IR-MONO`** — WB and neutral pickers, all 7 infrared monochrome mixes (`Mono from Red (IR)`, `IR Mono Swap BW`, Green/Blue/Yellow/Orange/IR filter mixes), plus `Exposure`, `Contrast`, `Whites`, `Blacks`, `Clarity`, and `Dehaze`.
  - **`AEROCHROME`** — camera-bound Chrome/Aerochrome DCP quick list (auto-detected from your imported `.dcp` profiles), `Profile Amount`, WB/neutral pickers, `Vibrance`/`Saturation`, `Foliage & Sky HSL` (Red, Orange, Yellow, Blue), and `Primary Calibration`.
- **46 clean-room sRGB infrared channel mixes (`HUB_IR_MIXES`):** `Swap Red and Blue (sRGB)` plus 7 built-in and 38 extended clean-room 3×3 mixes defined between sRGB primaries (`P · S · P⁻¹ · Base`, unlike a raw Rec.2020 swap which shifts azure skies toward teal). Selecting a mix replaces the active mix instead of stacking; clicking the active mix again restores the base mixer; out-of-range coefficients (`±200 %`) scale down uniformly and report the `EV` shift.
- **Automatic swap reset on profile change:** switching camera profiles (`develop.profile`) automatically resets any active IR swap or manual channel mixer (restorable in a single `Undo`), preventing colour-matrix mismatches across different profiles.
- **RGB Channel Mixer & Neutral Picker:** carried from upstream PR #3579 (taken before upstream review), allowing one-click neutralization of foliage or neutral targets in linear space.
- **Soft gamut compression for IR edits (`ir_gamut`):** ACES Reference Gamut Compression on CPU and GPU (applied only when a user DCP profile or non-neutral channel mixer is active). Prevents flat yellow/cyan fills when foliage white balance drives a channel deeply negative; ordinary photos render identically to upstream.
- **Camera-bound DCP profiles & Lightroom metadata recovery:** imported `.dcp` camera profiles match the photo's camera model (`UniqueCameraModel`) and are never applied to JPEGs or mismatched bodies. Includes `.lrcat` camera/lens recovery and a 64 KiB EXIF header fallback so imported Lightroom catalogs match DCPs immediately.
- **Compressed 12-bit Olympus ORF decoding:** ported from upstream PR #360 (kept in this fork with provenance notes; not part of upstream LightCraft).
- **Optional Nikon High Efficiency NEF (`HE` / `HE★`) decoding:** opt-in local LibRaw shim (`crates/nikon-he`, `--features lightcraft/nikon-he`, off by default).
- **Separate macOS app bundle (`LightCraft IR.app`, `art.irchrome.lightcraft-ir`):** installs cleanly alongside upstream LightCraft with upstream auto-update checks disabled.

## Infrared Processing Guide (Clean Whites & Reliable Swaps)

Infrared raw files often look bright red out of the camera because standard daylight WB and standard camera profiles hit the `2000 K` temperature floor before neutralizing infrared-rich channels. **Always establish white balance and neutral whites BEFORE applying a channel swap:**

### Why whites can drift blue or green
1. **Un-neutralized WB before swapping:** swapping R↔B on an orange/red raw turns residual warm cast directly into blue or cyan/green. If the standard `Temp` slider is pinned at `2000 K` on `Color`, you need an IR camera profile (`.dcp`) or the **Channel Mixer Neutral** picker.
2. **Stale swap after changing profiles:** changing from `Color` to an IR/Chrome DCP with an earlier swap still active mixes the wrong channels (LightCraft IR now resets the swap automatically whenever you switch profiles).
3. **Creative / unbalanced mixes:** mixes like `IR Red Look Alt` or `IR Purple and Gold` are intentionally non-neutral; use `Swap Red and Blue (sRGB)`, `Swaps`, `Swap Variants`, or `Fitted Looks` when you want clean neutral whites and clouds.

### Workflow 1 — False-Color IR (`550–720 nm`)
1. Open the **`Infrared`** tab (`Shift+I`) or expand **`Edit ▸ IR-FALSE-COLOR`**.
2. Choose an **IR `.dcp` profile** for your camera (if none is imported, use **Import DCP…**; if you stay on `Color`, use the **Channel Mixer Neutral** target icon instead of `W` alone).
3. Click the **White Balance (`W`)** eyedropper on white foliage, bright cloud highlights, or a neutral grey target. If whites still carry a tint, click the **Channel Mixer Neutral** eyedropper on a spot that should be pure white/grey.
4. Click **`Swap Red and Blue (sRGB)`** or pick a recipe from **`IR Mix Catalog`** / **`IR-FALSE-COLOR`** (`Swaps`, `Swap Variants`, `Fitted Looks`).
5. Fine-tune `Exposure`, `Vibrance`, and `Saturation`.

### Workflow 2 — Monochrome IR (`720 / 850 nm`)
1. Expand **`Edit ▸ IR-MONO`** (or select a mono mix in the **`Infrared`** tab).
2. Neutralize foliage/grey with `W` or the **Channel Mixer Neutral** picker.
3. Pick **`Mono from Red (IR)`** or one of the extended mixes (`IR Mono Swap BW`, `IR Mono IR Filter`, Green/Yellow/Orange/Blue filters).
4. Shape contrast with `Exposure`, `Contrast`, `Whites`, `Blacks`, `Clarity`, and `Dehaze`.

### Workflow 3 — Aerochrome / IRChrome
1. Expand **`Edit ▸ AEROCHROME`** (any previous swap is cleared automatically when you pick a profile).
2. Pick one of your imported **Chrome / Aerochrome** `.dcp` profiles and adjust `Amount`.
3. Set white balance on a neutral grey/cloud spot with `W` (or **Channel Mixer Neutral**), then refine foliage reds/oranges/yellows and sky blues with **`Foliage & Sky HSL`** and **`Primary Calibration`**.

### AI Masks (SAM 3) on Infrared Photos
If you use `Object` / `Describe` AI masks (SAM 3, installed via `./tools/install-sam3.sh`), run them **after** applying your IR DCP, foliage WB, and channel swap so sky and foliage have natural semantic contrast.

## Building from source (macOS)

Upstream's build and test instructions apply unchanged — see [`AGENTS.md`](AGENTS.md), [`docs/`](docs/) and upstream's [README](https://github.com/storytold/lightcraft/blob/main/README.md). The fork's own architecture and rules are documented in [`IR-FORK.md`](IR-FORK.md) and [`docs/IR-FORK-INVENTORY.md`](docs/IR-FORK-INVENTORY.md).

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
