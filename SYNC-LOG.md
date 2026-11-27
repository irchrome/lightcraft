# Upstream sync log (branch `ir`)

| Date | Upstream | `ir` before -> after | Conflicts | Notes |
|---|---|---|---|---|
| 2026-11-01 | c50b74df (main after v0.16.0) | fork created | — | branch `ir` started from upstream main |
| 2026-11-01 | v0.16.0-640-gc50b74df | a70b56bd -> a70b56bd | clean | |
| 2026-11-08 | b3b49987 (main after v0.19.0) | 11f55bb1 -> rebased; + PR #3579 squash | `.gitignore`, `CLAUDE.md` (upstream now `@AGENTS.md`), `README.md`, `orf.rs` (decoder tuple, duplicate bindings), ORF corpus test adapted | PR #3579: OPEN. Upstream test `v1_and_v2_render_as_before_v3` fails on macOS aarch64 on pure upstream too: skipped. README merge driver fixed (`keepfork`) |
| 2026-11-12 | 5b61ce1e (main, app 0.21.0) | c81c95f5 -> rebuilt by cherry-pick | `docs/whats-new.md` (both entries kept) | PR #3579: OPEN, head moved to 5415aaee (33 commits, merged upstream main 11-11): old squash dropped, re-squashed from the new head. Fork adds sRGB swap (item 4 in IR-FORK.md) |
| 2026-11-27 | 5973d956d (main, app 0.21.1) | 5872563eb (`ir-pre-sync-2026-11-27`) -> rebuilt on `upstream/main` | `craft-raw`/`craft-tiff`/`craft-develop` moved to `storytold/craft-libs` (`rev=b2d2a1e`): ORF (`olympus12`) & `nikon-he` moved to `crates/raw/src/ir_raw.rs` fallback; `ColorOps` & `ChannelMixer` bridged in `pipeline::colorops`; locales merged per-key | Added IR Edit sections (`IR-FALSE-COLOR`, `IR-MONO`, `AEROCHROME`), 46 `HUB_IR_MIXES` in `Infrared` tab, auto-reset of swap/mixer on `develop.profile`, and distinct IR mantis app icon (`assets/app-icon/lightcraft-ir.*`) |
