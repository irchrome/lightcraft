# IR-FORK-INVENTORY.md — Fork Features and Touched Upstream Files

Detailed technical inventory of what `irchrome/lightcraft` carries on top of upstream `storytold/lightcraft` (extracted from `IR-FORK.md` on 2026-11-23 to keep always-on context under 32 KB). Read this file when rebasing onto upstream (`tools/sync-upstream.sh`) or modifying fork-specific modules.

## 1. Fork Layer Docs & Hooks
- `IR-FORK.md`, `docs/IR-FORK-INVENTORY.md`, `SYNC-LOG.md`
- `tools/git-hooks/` (pre-commit refuses `.dcp`, `.cube`, raw files, `ir-private/`), `tools/sync-upstream.sh`, `tools/install-macos-app.sh`

## 2. Compressed 12-bit Olympus ORF Decoder (PR #360)
- **Files:** `crates/raw/src/vendor/olympus12.rs`, hook in `crates/raw/src/vendor/orf.rs`.
- **Status:** provenance disputed upstream (upstream withdrew its own ORF decoder in PR #3916 pending provenance review; objection to PR #360 unresolved). Kept here by owner's decision; **never offer upstream**.

## 3. RGB Channel Mixer / Neutral Picker / Red-Blue Swap (PR #3579)
- **Status:** squash of upstream PR #3579 taken before upstream review.
- **Sync rule:** check PR #3579 at every sync; once upstream merges it, drop the squash commit and re-apply item #4 below on top.

## 4. Infrared Mixes Between sRGB Primaries (`ir_mix`)
- **Commands:** `develop.mixerSwapRBsRGB` ("Swap Red and Blue (sRGB)") and `develop.mixerIrMix {mix}` ("More IR mixes": R↔G, G↔B, two rotations, R↔B 70% / 30%, mono from red).
- **Math & UX:** `P·S·P⁻¹·Base` between sRGB primaries (Rec.2020 swap turns gold pre-swap sky teal, h ≈ 216° vs ≈ 251°). Clicking a mix **replaces** the active IR mix (never stacks — prevents bug of 2026-11-12 on "Mono from Red"); clicking active mix again returns to `Base`; external mixer edits become new `Base`. Stored in `DevelopSettings::ir_mix` (`crates/develop/src/ir_mix.rs`, omitted from JSON when unset). Results past ±200% scale down and report `darker`.
- **Fork-owned files:** `crates/pipeline/src/ir_swap.rs`, `crates/engine/src/cmd/ir.rs`, `crates/ui-egui/src/panels/ir_mix.rs`, `crates/develop/src/ir_mix.rs`.
- **Upstream files touched (re-apply after PR #3579 re-squash):**
  - 1 line each in `crates/pipeline/src/lib.rs`, `crates/ui-egui/src/panels.rs`, `crates/ui-egui/src/panels/edit.rs`
  - 2 lines in `crates/engine/src/cmd.rs`
  - locale keys at end of `crates/ui-egui/locales/*.json`
  - `ir_mix` field (3 lines + `Default`) in `crates/develop/src/settings.rs`
  - `mod`/`pub use` + `Section::ChannelMixer` arm in `reset_section` in `crates/develop/src/lib.rs`
  - `"ir_mix"` key in Channel Mixer copy group in `crates/develop/src/presets.rs`

## 5. Camera-Bound DCP Profiles (`ir_profiles`)
- **Behavior:** imported `.dcp` is offered and accepted by `develop.profile` only for a raw matching `UniqueCameraModel` (`dcp_fits`: lowercase alphanumeric whole names, make + model, never prefixes). Paste, sync, Auto Sync, and presets keep a photo's own profile when carried profile doesn't fit. LUT profiles remain universal.
- **Fork-owned file:** `crates/engine/src/ir_profiles.rs`.
- **Upstream files touched:** 2 lines in `crates/engine/src/lib.rs` (`pub mod ir_profiles;` + `keep_own_camera_profile` at top of `Session::develop_op`), 1 in `crates/engine/src/cmd/develop.rs` (`refuse_unfit`), 1 in `crates/engine/src/presets.rs` (`imported_profiles_for`), 2 in `crates/ui-egui/src/panels/profiles.rs`, 2 in `crates/ui-egui/src/panels/edit/sliders.rs`, fixture helper in `crates/engine/src/tests/dcp.rs`.

## 6. Nikon High Efficiency NEF (HE / HE★) via Local LibRaw
- **Behavior:** off by default (`--features lightcraft/nikon-he,lightcraft-cli/nikon-he`). C shim over LibRaw public API (`libraw_open_buffer` + `libraw_unpack`, sensor samples only; levels/WB/colour/crop stay in LightCraft NEF reader).
- **Fork-owned crate:** `crates/nikon-he`.
- **Upstream files touched:** 1 `cfg` branch in `crates/raw/src/vendor/nef.rs` `compressed()`, feature `nikon-he` in `crates/raw`, `apps/lightcraft`, `apps/lightcraft-cli`, 1 row in `xtask/src/layers.rs`.
- **Local dependency (never committed):** LGPL-2.1/CDDL-1.0 LibRaw snapshot 202611+ built at `~/Documents/_vendor/LibRaw` (`make -f Makefile.dist lib/libraw.a`) or via `LIBRAW_DIR`. Verified bit-exact against Adobe DNG Converter (`crates/raw/tests/nikon_he.rs`, sample `~/Documents/_IR-raw-corpus/Zf/he-sample.nef`, DNG via `LC_NIKON_HE_DNG`). After building with feature, run `library.recheckPreviewOnly all=true` once.

## 7. Soft Gamut Compression for IR Edits (`ir_gamut`)
- **Behavior (owner decision 2026-11-13, IR photos only):** ACES Reference Gamut Compression (threshold 0.7, limit 2.2, power 1.2) runs when photo has user DCP (`dcp:` id) or non-neutral channel mixer, before mixer zero-clamp and output gamut map. Prevents flat yellow/cyan fills in foliage-balanced IR skies. Ordinary photos unaffected.
- **Files:** `crates/pipeline/src/ir_gamut.rs`; GPU twin `ir_compress` in `crates/gpu/src/wgsl/finish.wgsl`, `IR_GAMUT`/`IR_GAMUT_S` in `crates/gpu/src/params.rs`, `FinishParams::ir_gamut`; CPU = GPU within 1 code value (`equivalence.rs`). Does not bump `RENDER_CACHE_VERSION`: rebuild IR previews with `library.buildPreviews ids=[…] force=true`.

## 8. Fork Branding & Disconnected Upstream Updater
- **Files:** fork-owned `crates/ui-egui/src/ir_fork.rs`; touched `crates/ui-egui/src/dialogs.rs` (`about_body`), `crates/ui-egui/src/lib.rs` (`mod` + `UPDATE_APP` → `irchrome/lightcraft`), `apps/lightcraft/src/main.rs` (`set_startup_update_check(false)`), 3 keys at end of `crates/ui-egui/locales/*.json`. Update feature sentence in `ir_fork.rs` when fork feature set changes.
