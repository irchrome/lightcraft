# IR-FORK.md: the fork layer (read with upstream's CLAUDE.md / AGENTS.md)

This repository is `irchrome/lightcraft`, a fork of `storytold/lightcraft`. **It is in git; commits are allowed here** (one committing agent per repository). Planning, owner decisions and spike reports live outside this repository, in the owner's IR vault (`tasks/lightcraft-ir/`).

## Rules on top of upstream's

- **Work order follows the order of operations on a raw file:** camera colour under IR and a white balance that lands on foliage come first; channel swap, mixer and looks come after. Never demo a swap on a file whose white balance was not verified (owner rule, 2026-10-31).
- **What the fork carries on top of upstream (2026-11-12):** (1) this fork layer; (2) the compressed 12-bit Olympus ORF decoder ported from upstream PR #360 (`crates/raw/src/vendor/olympus12.rs`, hook in `crates/raw/src/vendor/orf.rs`): **provenance disputed upstream** (upstream withdrew its own ORF decoder in PR #3916 pending provenance review; the objection to PR #360 is unresolved), kept here by the owner's decision, never to be offered upstream; (3) a squash of upstream PR #3579 (RGB channel mixer, neutral picker, red/blue swap) taken before upstream review: at every sync check the PR; once upstream merges it, drop the squash commit. (4) **sRGB red/blue swap** (`develop.mixerSwapRBsRGB`, button "Swap Red and Blue (sRGB)" next to upstream's swap): the swap between sRGB primaries, `P·S·P⁻¹` on the mixer's Rec.2020, because the plain Rec.2020 swap turns a gold pre-swap sky teal (h ≈ 216° instead of ≈ 251°). Fork-owned files `crates/pipeline/src/ir_swap.rs`, `crates/engine/src/cmd/ir.rs`; upstream files touched: one line each in `crates/pipeline/src/lib.rs` and `crates/engine/src/cmd.rs` (two), the button in `crates/ui-egui/src/panels/edit.rs`, one key at the end of each `crates/ui-egui/locales/*.json`. Re-apply after each re-squash of PR #3579.
- **IR camera colour = user DCP profiles.** Upstream imports the user's own `.dcp` per photo (PR #1063); IR camera profiles per camera + filter are made by the fork's tooling from the owner's measurements and imported as user profiles. `.dcp` files never enter git (the hook refuses them), builds, examples or docs. Third-party DCPs (e.g. Rob Shea's) may be used only locally in the owner's own library (owner decision 2026-11-08).
- **Upstream files:** touch as few as possible. Planned IR-specific code lives in a new crate `crates/ir` (IR presets, DCP generator support, hotspot); the channel mixer and DCP import are upstream's. Known unavoidable edits: `xtask/src/layers.rs` (one row per new crate), `crates/ui-egui/locales/*.json` (keys at the end of each file).
- **Known upstream test failing on macOS aarch64:** `tests_light::v1_and_v2_render_as_before_v3` (platform-only golden hashes; fails on pure upstream too). Skip it locally with `-- --skip v1_and_v2_render_as_before_v3`.
- **Never edit** upstream's `docs/parity-checklist.md`, `docs/whats-new.md`, `docs/gaps.md`, `ROADMAP.md`, nor bump the shared numbers `RENDER_CACHE_VERSION`, `LOOK_VERSION`, catalog `VERSION`, `Process::LATEST` in the fork: upstream bumps them, and the fork takes theirs.
- **Clean room, as upstream (`CLAUDE.md`):** no Adobe profiles (`.dcp`), LUTs or camera matrices in the repository, no numbers derived from them in code. Owner decision 2026-11-01 allows *reading* third-party DCP matrices to understand the mechanism; the readout stays outside git.
- **Private material** (recipes of the `course` and `panel` groups, owner's raw corpus) never enters this public repository: keep it under `ir-private/` (gitignored) and `~/Documents/_IR-raw-corpus/` (outside git, sha256 manifests in `tests/corpus-manifests/`). The pre-commit hook in `tools/git-hooks/` refuses `.dcp`, `.cube`, raw files and anything under `ir-private/`.
- **Sync:** weekly `tools/sync-upstream.sh`; tag `ir-pre-sync-<date>` before every rebase; `cargo xtask ci --quick` after; push `ir` with `--force-with-lease` only. `main` is fast-forward only. Record every sync in `SYNC-LOG.md`.
- **Build:** `CARGO_TARGET_DIR=target/ir-agent cargo build --release -p lightcraft-cli` (own target dir, as upstream asks of agents). On the owner's macOS 26.6 machine the Command Line Tools' 27.0 SDK breaks the linker (`unknown architecture arm64e.x1` in `libSystem.tbd`); `~/.cargo/config.toml` sets `SDKROOT` to the 26.5 SDK, keep it. Control port for the desktop app: 18720 (never the default 7980).
- **Language:** code, commits and this repository in English; reports to the owner in Russian, in the IR vault.

## Install the hook and merge driver once per clone

`merge=keepfork` keeps the fork's `README.md` when `ir` is rebased onto upstream: during a rebase `%B` is the fork commit being replayed. Do not use `merge=ours` here, it keeps upstream's file in a rebase.

```
cd ~/Documents/lightcraft-ir
git config core.hooksPath tools/git-hooks
git config merge.keepfork.driver 'cp -f %B %A'
```
