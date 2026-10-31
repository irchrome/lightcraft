# IR-FORK.md: the fork layer (read with upstream's CLAUDE.md / AGENTS.md)

This repository is `irchrome/lightcraft`, a fork of `storytold/lightcraft`. **It is in git; commits are allowed here** (one committing agent per repository). Planning, owner decisions and spike reports live outside this repository, in the owner's IR vault (`tasks/lightcraft-ir/`).

## Rules on top of upstream's

- **Work order follows the order of operations on a raw file:** camera colour under IR and a white balance that lands on foliage come first; channel swap, mixer and looks come after. Never demo a swap on a file whose white balance was not verified (owner rule, 2026-10-31).
- **Upstream files:** touch as few as possible, in the quietest places. The agreed insertion points: `crates/develop/src/settings.rs` (one field), `crates/develop/src/controls.rs` (one section), `crates/pipeline/src/finish.rs` (compose into `FinishParams.calib`), `crates/ui-egui/src/panels/edit.rs` (one section block), `crates/ui-egui/locales/*.json` (keys at the end of each file), `xtask/src/layers.rs` (one row), `crates/engine/src/camera_profiles.rs` (profile key `model + filter`). Everything else IR-specific lives in `crates/ir`.
- **Never edit** upstream's `docs/parity-checklist.md`, `docs/whats-new.md`, `docs/gaps.md`, `ROADMAP.md`, nor bump the shared numbers `RENDER_CACHE_VERSION`, `LOOK_VERSION`, catalog `VERSION`, `Process::LATEST` in the fork: upstream bumps them, and the fork takes theirs.
- **Clean room, as upstream (`CLAUDE.md`):** no Adobe profiles (`.dcp`), LUTs or camera matrices in the repository, no numbers derived from them in code. Owner decision 2026-11-01 allows *reading* third-party DCP matrices to understand the mechanism; the readout stays outside git.
- **Private material** (recipes of the `course` and `panel` groups, owner's raw corpus) never enters this public repository: keep it under `ir-private/` (gitignored) and `~/Documents/_IR-raw-corpus/` (outside git, sha256 manifests in `tests/corpus-manifests/`). The pre-commit hook in `tools/git-hooks/` refuses `.dcp`, `.cube`, raw files and anything under `ir-private/`.
- **Sync:** weekly `tools/sync-upstream.sh`; tag `ir-pre-sync-<date>` before every rebase; `cargo xtask ci --quick` after; push `ir` with `--force-with-lease` only. `main` is fast-forward only. Record every sync in `SYNC-LOG.md`.
- **Build:** `CARGO_TARGET_DIR=target/ir-agent cargo build --release -p lightcraft-cli` (own target dir, as upstream asks of agents). On the owner's macOS 26.6 machine the Command Line Tools' 27.0 SDK breaks the linker (`unknown architecture arm64e.x1` in `libSystem.tbd`); `~/.cargo/config.toml` sets `SDKROOT` to the 26.5 SDK, keep it. Control port for the desktop app: 18720 (never the default 7980).
- **Language:** code, commits and this repository in English; reports to the owner in Russian, in the IR vault.

## Install the hook once per clone

```
cd ~/Documents/lightcraft-ir
git config core.hooksPath tools/git-hooks
git config merge.ours.driver true
```
