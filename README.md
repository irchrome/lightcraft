# LightCraft IR

A maintained fork of [storytold/lightcraft](https://github.com/storytold/lightcraft), the open-source,
pure-Rust photo library and raw developer, with **native infrared (IR) raw processing**: camera colour
under IR filters (590 / 665 / 720 / 850 nm, full spectrum), white balance that lands on foliage, channel
swap and mixing in scene-linear light, IR looks as presets, lens hotspot correction.

Upstream's README, docs, build and test instructions apply unchanged: see
[`AGENTS.md`](AGENTS.md), [`docs/`](docs/) and upstream's
[README](https://github.com/storytold/lightcraft/blob/main/README.md). The fork's own layer is
described in [`IR-FORK.md`](IR-FORK.md).

## Branches

| Branch | What it is |
|---|---|
| `main` | an exact mirror of upstream `main`, fast-forward only |
| `ir` | upstream `main` plus the IR patch series, rebased on upstream weekly (`tools/sync-upstream.sh`, log in `SYNC-LOG.md`). History of `ir` is rewritten on every sync; a tag `ir-pre-sync-<date>` marks the state before each rebase |

## Compatibility

Develop settings written by this fork carry an extra `ir` field. Upstream LightCraft ignores unknown
fields and **drops it silently when it saves**, so do not open one library with both builds.

## Licence and trademarks

MIT OR Apache-2.0, as upstream (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`). The ArtCraft name, wordmark
and logos are trademarks of the ArtCraft Team and are not part of this fork; `docs/brand/` was removed
as upstream's licence requires. This fork is not affiliated with or endorsed by ArtCraft.

IR camera matrices and presets shipped here are the fork author's own measurements (see
`assets/ATTRIBUTION.md`). No Adobe profile, lens profile or LUT is included or derived from.
