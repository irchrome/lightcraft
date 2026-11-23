# IR-FORK.md — Fork Layer (read with upstream `AGENTS.md`)

Repository: `irchrome/lightcraft` (fork of `storytold/lightcraft`). **In git; commits allowed** (one committing agent per repo). Planning, owner decisions, and spike reports live in the IR vault (`~/Documents/IR/tasks/lightcraft-ir/`).

Detailed inventory of the 8 fork features and touched upstream files: read [`docs/IR-FORK-INVENTORY.md`](docs/IR-FORK-INVENTORY.md) before rebasing or touching fork modules.

## Standing Rules on Top of Upstream

- **Raw processing order:** camera colour under IR and foliage-balanced WB come first; channel swap, mixer, and looks come after. Never demo a swap on a file with unverified WB (owner rule, 2026-10-31).
- **Minimal upstream diff:** keep new IR code in fork-owned modules/crates (planned `crates/ir`). Unavoidable upstream edits: `xtask/src/layers.rs` (1 row per crate), `crates/ui-egui/locales/*.json` (keys at end). **Never edit** upstream `docs/parity-checklist.md`, `docs/whats-new.md`, `docs/gaps.md`, `ROADMAP.md`, nor bump `RENDER_CACHE_VERSION`, `LOOK_VERSION`, catalog `VERSION`, or `Process::LATEST`.
- **Clean room & private data:** no `.dcp`, `.cube`, Adobe profiles/matrices, raw files, or `ir-private/` in git (enforced by `tools/git-hooks/` pre-commit). Reading third-party DCP matrices locally is allowed (owner decision 2026-11-01), output stays outside git. Raw test corpus lives in `~/Documents/_IR-raw-corpus/` (sha256 manifests in `tests/corpus-manifests/`). Local LibRaw (`~/Documents/_vendor/LibRaw`, LGPL) never enters git.
- **Never upstream:** Olympus 12-bit ORF decoder (`olympus12.rs`, provenance disputed upstream).
- **Known macOS aarch64 upstream test failure:** skip `tests_light::v1_and_v2_render_as_before_v3` locally (`-- --skip v1_and_v2_render_as_before_v3`).
- **Sync:** weekly `tools/sync-upstream.sh`; tag `ir-pre-sync-<date>` before rebase; `cargo xtask ci --quick` after; push `ir` with `--force-with-lease` only; `main` is fast-forward only; log every sync in `SYNC-LOG.md`.
- **Build & run:**
  - `CARGO_TARGET_DIR=target/ir-agent cargo build --release -p lightcraft -p lightcraft-cli --features lightcraft/nikon-he,lightcraft-cli/nikon-he` (omit `--features` on machines without LibRaw).
  - macOS app / public DMG (without `nikon-he`): `tools/install-macos-app.sh`.
  - Keep `SDKROOT` pointing to macOS 26.5 SDK in `~/.cargo/config.toml` (27.0 SDK breaks linker on macOS 26.6).
  - Desktop control port: `18720` (never default `7980`).
- **Language:** code, commits, and repo docs in English; owner reports in Russian (in IR vault).

## Once Per Clone (Hooks & Merge Driver)

`merge=keepfork` keeps the fork's `README.md` during rebase (do not use `merge=ours`).

```bash
cd ~/Documents/lightcraft-ir
git config core.hooksPath tools/git-hooks
git config merge.keepfork.driver 'cp -f %B %A'
```
